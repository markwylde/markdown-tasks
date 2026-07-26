//! Filesystem watching and refresh coordination for the interactive UI.
//!
//! Native watcher callbacks only enqueue small messages.  Debouncing and all
//! scanning happen outside the callback, so a slow scan cannot stall either
//! the platform watcher or the UI event loop.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, TryRecvError, TrySendError, bounded};
use notify::{
    Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcherTrait,
    event::{CreateKind, ModifyKind, RemoveKind},
};

use crate::error::MdtError;

/// Quiet period used to coalesce a burst of filesystem events.
pub const DEBOUNCE_DURATION: Duration = Duration::from_millis(200);
/// How long a successful refresh is described as recent.
pub const UPDATED_STATUS_DURATION: Duration = Duration::from_secs(2);
/// Default capacity of the native watcher callback queue.
pub const WATCH_CHANNEL_CAPACITY: usize = 256;

/// A clock makes debounce and status transitions deterministic in tests.
pub trait Clock {
    fn now(&self) -> Instant;
}

/// Production monotonic clock.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// A normalized message emitted by a filesystem watcher.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WatchEvent {
    /// One or more paths may have changed.
    Changed(Vec<PathBuf>),
    /// One or more paths may be directories. Their extensions must not be used
    /// to decide relevance because dotted directory names are valid.
    ChangedDirectories(Vec<PathBuf>),
    /// The backend lost events and a full scan is required.
    Rescan,
    /// Watching failed, but manual refresh remains available.
    Error(String),
}

impl WatchEvent {
    #[must_use]
    pub fn requires_refresh(&self) -> bool {
        matches!(
            self,
            Self::Changed(_) | Self::ChangedDirectories(_) | Self::Rescan
        )
    }
}

/// Small abstraction around a native or future polling watcher.
pub trait Watcher: Send {
    /// Non-blockingly receive the next normalized watcher message.
    ///
    /// # Errors
    ///
    /// Returns `Empty` when no message is ready or `Disconnected` if the
    /// watcher backend stopped.
    fn try_recv(&self) -> Result<WatchEvent, TryRecvError>;
}

/// Production `notify` watcher.
///
/// The bounded queue protects the UI from an unbounded native event burst.
/// If it fills, an atomic overflow flag preserves the important information:
/// the consumer receives one `Rescan` after it has made room.
pub struct NotifyWatcher {
    _watcher: RecommendedWatcher,
    receiver: Receiver<WatchEvent>,
    overflowed: Arc<AtomicBool>,
}

impl NotifyWatcher {
    /// Start watching `target`.
    ///
    /// Files are watched through their parent so delete/recreate is recoverable.
    /// Directories are watched recursively and their parent is watched
    /// non-recursively so recreation of a deleted explicit root can be noticed.
    ///
    /// # Errors
    ///
    /// Returns a watcher error when the native backend cannot be created or attached.
    pub fn new(target: &Path) -> Result<Self, MdtError> {
        Self::with_capacity(target, WATCH_CHANNEL_CAPACITY)
    }

    /// As [`NotifyWatcher::new`], with an explicit queue capacity.
    ///
    /// # Errors
    ///
    /// Returns a watcher error for a zero capacity or native backend failure.
    pub fn with_capacity(target: &Path, capacity: usize) -> Result<Self, MdtError> {
        if capacity == 0 {
            return Err(MdtError::Watch(
                "watch channel capacity must be greater than zero".into(),
            ));
        }

        let (sender, receiver) = bounded(capacity);
        let overflowed = Arc::new(AtomicBool::new(false));
        let callback_overflowed = Arc::clone(&overflowed);
        let mut watcher = RecommendedWatcher::new(
            move |result| enqueue_notify_result(result, &sender, &callback_overflowed),
            Config::default(),
        )
        .map_err(|error| MdtError::Watch(error.to_string()))?;

        for (path, mode) in watch_locations(target) {
            watcher
                .watch(&path, mode)
                .map_err(|error| MdtError::Watch(error.to_string()))?;
        }

        Ok(Self {
            _watcher: watcher,
            receiver,
            overflowed,
        })
    }

    /// Non-blockingly receive the next normalized watcher message.
    ///
    /// # Errors
    ///
    /// Returns `Empty` or `Disconnected` following channel semantics.
    pub fn try_recv(&self) -> Result<WatchEvent, TryRecvError> {
        receive_watch_event(&self.receiver, &self.overflowed)
    }
}

impl Watcher for NotifyWatcher {
    fn try_recv(&self) -> Result<WatchEvent, TryRecvError> {
        self.try_recv()
    }
}

fn receive_watch_event(
    receiver: &Receiver<WatchEvent>,
    overflowed: &AtomicBool,
) -> Result<WatchEvent, TryRecvError> {
    match receiver.try_recv() {
        Ok(event) => Ok(event),
        Err(TryRecvError::Empty) if overflowed.swap(false, Ordering::AcqRel) => {
            Ok(WatchEvent::Rescan)
        }
        result => result,
    }
}

fn enqueue_notify_result(
    result: notify::Result<Event>,
    sender: &Sender<WatchEvent>,
    overflowed: &AtomicBool,
) {
    let message = match result {
        Ok(event) if event.need_rescan() => WatchEvent::Rescan,
        Ok(event) if matches!(event.kind, EventKind::Any | EventKind::Other) => WatchEvent::Rescan,
        Ok(event) if is_directory_or_rename_kind(event.kind) => {
            WatchEvent::ChangedDirectories(event.paths)
        }
        Ok(event) if is_refresh_kind(event.kind) => WatchEvent::Changed(event.paths),
        Ok(_) => return,
        Err(error) => WatchEvent::Error(error.to_string()),
    };

    if matches!(sender.try_send(message), Err(TrySendError::Full(_))) {
        overflowed.store(true, Ordering::Release);
    }
}

fn watch_locations(target: &Path) -> Vec<(PathBuf, RecursiveMode)> {
    if target.is_file() {
        let parent = usable_parent(target);
        return vec![(parent.to_path_buf(), RecursiveMode::NonRecursive)];
    }

    if target.is_dir() {
        let mut locations = vec![(target.to_path_buf(), RecursiveMode::Recursive)];
        let parent = usable_parent(target);
        if parent != target {
            locations.push((parent.to_path_buf(), RecursiveMode::NonRecursive));
        }
        return locations;
    }

    let mut existing = target.to_path_buf();
    while !existing.exists() {
        let parent = usable_parent(&existing);
        if parent == existing {
            existing = PathBuf::from(".");
            break;
        }
        existing = parent.to_path_buf();
    }
    vec![(existing, RecursiveMode::Recursive)]
}

fn usable_parent(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| {
            if path.has_root() {
                path
            } else {
                Path::new(".")
            }
        })
}

fn is_refresh_kind(kind: EventKind) -> bool {
    matches!(
        kind,
        EventKind::Create(
            CreateKind::Any | CreateKind::File | CreateKind::Folder | CreateKind::Other
        ) | EventKind::Modify(
            ModifyKind::Any
                | ModifyKind::Data(_)
                | ModifyKind::Metadata(_)
                | ModifyKind::Name(_)
                | ModifyKind::Other
        ) | EventKind::Remove(
            RemoveKind::Any | RemoveKind::File | RemoveKind::Folder | RemoveKind::Other
        ) | EventKind::Any
            | EventKind::Other
    )
}

fn is_directory_or_rename_kind(kind: EventKind) -> bool {
    matches!(
        kind,
        EventKind::Create(CreateKind::Folder)
            | EventKind::Remove(RemoveKind::Folder)
            | EventKind::Modify(ModifyKind::Name(_))
    )
}

/// Determines which normalized native events can change a Markdown snapshot.
#[derive(Clone, Debug)]
pub struct EventFilter {
    root: PathBuf,
    canonical_root: Option<PathBuf>,
    root_is_file: bool,
    ignored_directories: Vec<String>,
}

impl EventFilter {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>, root_is_file: bool) -> Self {
        let root = root.into();
        let canonical_root = canonicalize_with_missing_tail(&root).filter(|path| path != &root);
        Self {
            root,
            canonical_root,
            root_is_file,
            ignored_directories: vec![
                ".git".into(),
                ".hg".into(),
                ".svn".into(),
                "node_modules".into(),
                "target".into(),
            ],
        }
    }

    /// Replace ignored directory names and root-relative paths, normally using
    /// discovery settings.
    #[must_use]
    pub fn with_ignored_directories<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.ignored_directories = names.into_iter().map(Into::into).collect();
        self
    }

    /// Filter irrelevant paths from an event; errors and rescan signals pass
    /// through unchanged.
    #[must_use]
    pub fn filter(&self, event: WatchEvent) -> Option<WatchEvent> {
        match event {
            WatchEvent::Changed(paths) => {
                let relevant: Vec<_> = paths
                    .into_iter()
                    .filter(|path| self.is_relevant_path(path))
                    .collect();
                (!relevant.is_empty()).then_some(WatchEvent::Changed(relevant))
            }
            WatchEvent::ChangedDirectories(paths) => {
                let relevant = paths
                    .into_iter()
                    .filter(|path| self.is_relevant_directory_path(path))
                    .collect::<Vec<_>>();
                (!relevant.is_empty()).then_some(WatchEvent::ChangedDirectories(relevant))
            }
            other => Some(other),
        }
    }

    #[must_use]
    pub fn is_relevant_path(&self, path: &Path) -> bool {
        if self.root_is_file {
            return path == self.root || self.canonical_root.as_deref() == Some(path);
        }
        let base = if path == self.root || path.starts_with(&self.root) {
            &self.root
        } else if let Some(canonical) = self
            .canonical_root
            .as_ref()
            .filter(|root| path == root.as_path() || path.starts_with(root))
        {
            canonical
        } else {
            return false;
        };

        if !self.is_relevant_relative(path.strip_prefix(base).unwrap_or(path)) {
            return false;
        }

        // Extensionless paths may be directories (including removed ones), and
        // directory changes can add or remove supported Markdown descendants.
        match path.extension().and_then(|extension| extension.to_str()) {
            None => true,
            Some(extension) => ["md", "markdown", "mdown", "mkd"]
                .iter()
                .any(|supported| extension.eq_ignore_ascii_case(supported)),
        }
    }

    fn is_relevant_directory_path(&self, path: &Path) -> bool {
        if self.root_is_file {
            return path == self.root || self.canonical_root.as_deref() == Some(path);
        }
        let base = if path == self.root || path.starts_with(&self.root) {
            &self.root
        } else if let Some(canonical) = self
            .canonical_root
            .as_ref()
            .filter(|root| path == root.as_path() || path.starts_with(root))
        {
            canonical
        } else {
            return false;
        };
        self.is_relevant_relative(path.strip_prefix(base).unwrap_or(path))
    }

    fn is_relevant_relative(&self, relative: &Path) -> bool {
        let relative_components = relative
            .components()
            .map(|component| component.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        if self.ignored_directories.iter().any(|ignored| {
            let ignored_components = ignored
                .split(['/', '\\'])
                .filter(|component| !component.is_empty() && *component != ".")
                .collect::<Vec<_>>();
            if ignored_components.len() == 1 {
                relative_components
                    .iter()
                    .any(|component| component == ignored_components[0])
            } else {
                relative_components
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .starts_with(&ignored_components)
            }
        }) {
            return false;
        }
        true
    }
}

fn canonicalize_with_missing_tail(path: &Path) -> Option<PathBuf> {
    let mut existing = path;
    while !existing.exists() {
        existing = existing.parent()?;
    }
    let canonical = existing.canonicalize().ok()?;
    let tail = path.strip_prefix(existing).ok()?;
    Some(canonical.join(tail))
}

/// Quiet-period debounce state. Callers provide `now`, avoiding internal sleeps.
#[derive(Clone, Debug, Default)]
pub struct Debouncer {
    deadline: Option<Instant>,
}

impl Debouncer {
    pub fn record(&mut self, now: Instant) {
        self.deadline = Some(now + DEBOUNCE_DURATION);
    }

    pub fn clear(&mut self) {
        self.deadline = None;
    }

    #[must_use]
    pub fn deadline(&self) -> Option<Instant> {
        self.deadline
    }

    /// Returns true once after the quiet period has elapsed.
    pub fn take_ready(&mut self, now: Instant) -> bool {
        if self.deadline.is_some_and(|deadline| now >= deadline) {
            self.deadline = None;
            true
        } else {
            false
        }
    }
}

/// A scan failure that controls persistent UI status.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScanError {
    TargetMissing,
    Failed(String),
}

impl fmt::Display for ScanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TargetMissing => formatter.write_str("target missing"),
            Self::Failed(message) => formatter.write_str(message),
        }
    }
}

/// User-visible state of live refresh.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RefreshStatus {
    UpToDate,
    Refreshing,
    UpdatedJustNow,
    WatcherError(String),
    TargetMissing,
    RefreshError(String),
    ShuttingDown,
}

impl fmt::Display for RefreshStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UpToDate => formatter.write_str("up to date"),
            Self::Refreshing => formatter.write_str("refreshing…"),
            Self::UpdatedJustNow => formatter.write_str("updated just now"),
            Self::WatcherError(message) => {
                write!(formatter, "watcher error: {message}; press r to refresh")
            }
            Self::TargetMissing => formatter.write_str("target missing"),
            Self::RefreshError(message) => write!(formatter, "refresh failed: {message}"),
            Self::ShuttingDown => formatter.write_str("shutting down"),
        }
    }
}

/// Messages for the UI. A snapshot is emitted only after a complete successful scan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RefreshEvent<T> {
    Snapshot(T),
    Status(RefreshStatus),
}

/// Non-blocking scan worker abstraction used by the refresh coordinator.
pub trait ScanWorker<T>: Send {
    /// Start a scan without blocking.
    ///
    /// # Errors
    ///
    /// Returns [`WorkerUnavailable`] when a scan is already queued or the worker stopped.
    fn try_start(&mut self) -> Result<(), WorkerUnavailable>;
    /// Poll for a completed scan.
    ///
    /// # Errors
    ///
    /// Returns `Empty` while work continues or `Disconnected` if the worker stopped.
    fn try_result(&mut self) -> Result<Result<T, ScanError>, TryRecvError>;
    fn shutdown(&mut self);
}

/// Indicates that a scan worker cannot accept a new command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerUnavailable;

/// Cooperative cancellation signal for scanners that can stop between steps.
#[derive(Clone, Debug)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// A production worker backed by one reusable scan thread and bounded channels.
pub struct ThreadScanWorker<T> {
    commands: Option<Sender<()>>,
    results: Receiver<Result<T, ScanError>>,
    stopping: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl<T: Send + 'static> ThreadScanWorker<T> {
    pub fn new<F>(scan: F) -> Self
    where
        F: Fn() -> Result<T, ScanError> + Send + 'static,
    {
        Self::new_cancellable(move |_| scan())
    }

    /// Create a worker whose scanner can observe shutdown cooperatively.
    pub fn new_cancellable<F>(scan: F) -> Self
    where
        F: Fn(&CancellationToken) -> Result<T, ScanError> + Send + 'static,
    {
        let (command_sender, command_receiver) = bounded::<()>(1);
        let (result_sender, result_receiver) = bounded(1);
        let stopping = Arc::new(AtomicBool::new(false));
        let running = Arc::new(AtomicBool::new(false));
        let worker_stopping = Arc::clone(&stopping);
        let worker_running = Arc::clone(&running);
        let cancellation = CancellationToken(Arc::clone(&stopping));
        let handle = thread::spawn(move || {
            while command_receiver.recv().is_ok() {
                let result = scan(&cancellation);
                worker_running.store(false, Ordering::Release);
                if worker_stopping.load(Ordering::Acquire) {
                    break;
                }
                if result_sender.send(result).is_err() {
                    break;
                }
            }
        });

        Self {
            commands: Some(command_sender),
            results: result_receiver,
            stopping,
            running,
            handle: Some(handle),
        }
    }

    fn join(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl<T: Send + 'static> ScanWorker<T> for ThreadScanWorker<T> {
    fn try_start(&mut self) -> Result<(), WorkerUnavailable> {
        self.running.store(true, Ordering::Release);
        let result = self
            .commands
            .as_ref()
            .map_or(Err(WorkerUnavailable), |sender| {
                sender.try_send(()).map_err(|_| WorkerUnavailable)
            });
        if result.is_err() {
            self.running.store(false, Ordering::Release);
        }
        result
    }

    fn try_result(&mut self) -> Result<Result<T, ScanError>, TryRecvError> {
        self.results.try_recv()
    }

    fn shutdown(&mut self) {
        self.stopping.store(true, Ordering::Release);
        self.commands.take();
        self.join();
    }
}

impl<T> Drop for ThreadScanWorker<T> {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Release);
        self.commands.take();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Coordinates debounce, manual refresh, single-flight scans, and UI messages.
pub struct RefreshCoordinator<T, W: ScanWorker<T>> {
    worker: W,
    debouncer: Debouncer,
    scanning: bool,
    pending: bool,
    accepting: bool,
    updated_until: Option<Instant>,
    events: Vec<RefreshEvent<T>>,
}

impl<T, W: ScanWorker<T>> RefreshCoordinator<T, W> {
    #[must_use]
    pub fn new(worker: W) -> Self {
        Self {
            worker,
            debouncer: Debouncer::default(),
            scanning: false,
            pending: false,
            accepting: true,
            updated_until: None,
            events: Vec::new(),
        }
    }

    /// Queue a debounced filesystem refresh.
    pub fn filesystem_event(&mut self, now: Instant) {
        if !self.accepting {
            return;
        }
        if self.scanning {
            self.pending = true;
        } else {
            self.debouncer.record(now);
        }
    }

    /// Queue a filesystem refresh using an injected clock.
    pub fn filesystem_event_with_clock(&mut self, clock: &impl Clock) {
        self.filesystem_event(clock.now());
    }

    /// Classify one watcher message and update refresh/status state.
    pub fn watcher_event(&mut self, event: WatchEvent, filter: &EventFilter, now: Instant) {
        let Some(event) = filter.filter(event) else {
            return;
        };
        match event {
            WatchEvent::Changed(_) | WatchEvent::ChangedDirectories(_) | WatchEvent::Rescan => {
                self.filesystem_event(now);
            }
            WatchEvent::Error(message) => self.watcher_error(message),
        }
    }

    /// Drain all watcher messages currently available without blocking the UI.
    ///
    /// Returns the number of messages handled. A disconnected watcher becomes
    /// an actionable persistent status.
    pub fn drain_watcher(
        &mut self,
        watcher: &impl Watcher,
        filter: &EventFilter,
        now: Instant,
    ) -> usize {
        let mut handled = 0;
        loop {
            match watcher.try_recv() {
                Ok(event) => {
                    handled += 1;
                    self.watcher_event(event, filter, now);
                }
                Err(TryRecvError::Empty) => return handled,
                Err(TryRecvError::Disconnected) => {
                    self.watcher_error("watcher stopped");
                    return handled;
                }
            }
        }
    }

    /// Queue an immediate user refresh. Repeated requests during a scan coalesce.
    pub fn manual_refresh(&mut self) {
        if !self.accepting {
            return;
        }
        self.debouncer.clear();
        if self.scanning {
            self.pending = true;
        } else {
            self.start_scan();
        }
    }

    /// Record a watcher failure without stopping manual refresh.
    pub fn watcher_error(&mut self, message: impl Into<String>) {
        if self.accepting {
            self.events
                .push(RefreshEvent::Status(RefreshStatus::WatcherError(
                    message.into(),
                )));
        }
    }

    /// Advance debounce/status time and collect any completed worker result.
    pub fn poll(&mut self, now: Instant) {
        if !self.accepting {
            return;
        }
        if !self.scanning && self.debouncer.take_ready(now) {
            self.start_scan();
        }

        if self.scanning {
            match self.worker.try_result() {
                Ok(result) => self.finish_scan(result, now),
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    self.scanning = false;
                    self.pending = false;
                    self.events
                        .push(RefreshEvent::Status(RefreshStatus::RefreshError(
                            "scan worker stopped".into(),
                        )));
                }
            }
        } else if self
            .updated_until
            .is_some_and(|updated_until| now >= updated_until)
        {
            self.updated_until = None;
            self.events
                .push(RefreshEvent::Status(RefreshStatus::UpToDate));
        }
    }

    /// Advance the coordinator using an injected clock.
    pub fn poll_with_clock(&mut self, clock: &impl Clock) {
        self.poll(clock.now());
    }

    /// Drain UI messages accumulated since the last call.
    pub fn drain_events(&mut self) -> impl Iterator<Item = RefreshEvent<T>> + '_ {
        self.events.drain(..)
    }

    #[must_use]
    pub const fn is_scanning(&self) -> bool {
        self.scanning
    }

    #[must_use]
    pub fn has_pending_refresh(&self) -> bool {
        self.pending || self.debouncer.deadline().is_some()
    }

    /// Stop accepting requests and make late worker messages unobservable.
    pub fn shutdown(&mut self) {
        if !self.accepting {
            return;
        }
        self.accepting = false;
        self.pending = false;
        self.debouncer.clear();
        self.events.clear();
        self.events
            .push(RefreshEvent::Status(RefreshStatus::ShuttingDown));
        self.worker.shutdown();
    }

    fn start_scan(&mut self) {
        if self.worker.try_start().is_ok() {
            self.scanning = true;
            self.updated_until = None;
            self.events
                .push(RefreshEvent::Status(RefreshStatus::Refreshing));
        } else {
            self.events
                .push(RefreshEvent::Status(RefreshStatus::RefreshError(
                    "scan worker unavailable".into(),
                )));
        }
    }

    fn finish_scan(&mut self, result: Result<T, ScanError>, now: Instant) {
        self.scanning = false;
        match result {
            Ok(snapshot) => {
                self.events.push(RefreshEvent::Snapshot(snapshot));
                self.events
                    .push(RefreshEvent::Status(RefreshStatus::UpdatedJustNow));
                self.updated_until = Some(now + UPDATED_STATUS_DURATION);
            }
            Err(ScanError::TargetMissing) => {
                self.events
                    .push(RefreshEvent::Status(RefreshStatus::TargetMissing));
            }
            Err(ScanError::Failed(message)) => {
                self.events
                    .push(RefreshEvent::Status(RefreshStatus::RefreshError(message)));
            }
        }

        if self.pending {
            self.pending = false;
            self.start_scan();
        }
    }
}

impl<T, W: ScanWorker<T>> Drop for RefreshCoordinator<T, W> {
    fn drop(&mut self) {
        self.worker.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::fs;
    use std::sync::Mutex;
    use std::time::SystemTime;

    use notify::event::{DataChange, RenameMode};

    use super::*;

    struct FakeWorker<T> {
        starts: usize,
        results: VecDeque<Result<T, ScanError>>,
        shutdown: bool,
    }

    struct FakeWatcher(Mutex<VecDeque<WatchEvent>>);

    impl Watcher for FakeWatcher {
        fn try_recv(&self) -> Result<WatchEvent, TryRecvError> {
            self.0
                .lock()
                .expect("watcher queue")
                .pop_front()
                .ok_or(TryRecvError::Empty)
        }
    }

    impl<T> FakeWorker<T> {
        fn new(results: impl IntoIterator<Item = Result<T, ScanError>>) -> Self {
            Self {
                starts: 0,
                results: results.into_iter().collect(),
                shutdown: false,
            }
        }
    }

    impl<T: Send> ScanWorker<T> for FakeWorker<T> {
        fn try_start(&mut self) -> Result<(), WorkerUnavailable> {
            self.starts += 1;
            Ok(())
        }

        fn try_result(&mut self) -> Result<Result<T, ScanError>, TryRecvError> {
            self.results.pop_front().ok_or(TryRecvError::Empty)
        }

        fn shutdown(&mut self) {
            self.shutdown = true;
        }
    }

    fn statuses<T: Send>(
        coordinator: &mut RefreshCoordinator<T, FakeWorker<T>>,
    ) -> Vec<RefreshStatus> {
        coordinator
            .drain_events()
            .filter_map(|event| match event {
                RefreshEvent::Status(status) => Some(status),
                RefreshEvent::Snapshot(_) => None,
            })
            .collect()
    }

    #[test]
    fn event_kinds_include_create_modify_remove_rename_and_rescan() {
        assert!(is_refresh_kind(EventKind::Create(CreateKind::File)));
        assert!(is_refresh_kind(EventKind::Modify(ModifyKind::Data(
            DataChange::Content
        ))));
        assert!(is_refresh_kind(EventKind::Modify(ModifyKind::Name(
            RenameMode::Both
        ))));
        assert!(is_refresh_kind(EventKind::Remove(RemoveKind::Folder)));
        assert!(is_refresh_kind(EventKind::Other));
        assert!(!is_refresh_kind(EventKind::Access(
            notify::event::AccessKind::Any
        )));
    }

    #[test]
    fn event_filter_keeps_markdown_and_directory_events_only() {
        let filter = EventFilter::new("/repo/docs", false);
        assert!(filter.is_relevant_path(Path::new("/repo/docs/guide.md")));
        assert!(filter.is_relevant_path(Path::new("/repo/docs/GUIDE.MARKDOWN")));
        assert!(filter.is_relevant_path(Path::new("/repo/docs/guide.MDOWN")));
        assert!(filter.is_relevant_path(Path::new("/repo/docs/guide.mkd")));
        assert!(filter.is_relevant_path(Path::new("/repo/docs/new-directory")));
        assert!(!filter.is_relevant_path(Path::new("/repo/docs/image.png")));
        assert!(!filter.is_relevant_path(Path::new("/repo/other/task.md")));
        assert!(!filter.is_relevant_path(Path::new("/repo/docs/.git/task.md")));
        assert!(!filter.is_relevant_path(Path::new("/repo/docs/target/task.md")));
    }

    #[test]
    fn event_filter_honors_root_relative_ignore_paths() {
        let filter = EventFilter::new("/repo/docs", false).with_ignored_directories([
            ".git",
            "generated/api",
            "nested\\windows-style",
        ]);
        assert!(!filter.is_relevant_path(Path::new("/repo/docs/generated/api/task.md")));
        assert!(filter.is_relevant_path(Path::new("/repo/docs/other/generated/api/task.md")));
        assert!(!filter.is_relevant_path(Path::new("/repo/docs/nested/windows-style/task.md")));
    }

    #[test]
    fn file_filter_ignores_sibling_changes_but_keeps_recreation() {
        let filter = EventFilter::new("/repo/tasks.md", true);
        assert!(filter.is_relevant_path(Path::new("/repo/tasks.md")));
        assert!(!filter.is_relevant_path(Path::new("/repo/other.md")));
    }

    #[test]
    fn changed_event_is_dropped_when_all_paths_are_ignored() {
        let filter = EventFilter::new("/repo", false);
        assert_eq!(
            filter.filter(WatchEvent::Changed(vec![
                "/repo/.git/index".into(),
                "/repo/readme.txt".into()
            ])),
            None
        );
        assert_eq!(filter.filter(WatchEvent::Rescan), Some(WatchEvent::Rescan));
    }

    #[test]
    fn coordinator_drains_filtered_watcher_events_without_blocking() {
        let now = Instant::now();
        let watcher = FakeWatcher(Mutex::new(VecDeque::from([
            WatchEvent::Changed(vec!["/repo/image.png".into()]),
            WatchEvent::Changed(vec!["/repo/tasks.md".into()]),
            WatchEvent::Error("backend failed".into()),
        ])));
        let worker = FakeWorker::new([]);
        let mut coordinator = RefreshCoordinator::<usize, _>::new(worker);
        assert_eq!(
            coordinator.drain_watcher(&watcher, &EventFilter::new("/repo", false), now),
            3
        );
        assert!(coordinator.has_pending_refresh());
        assert_eq!(
            statuses(&mut coordinator),
            vec![RefreshStatus::WatcherError("backend failed".into())]
        );
    }

    #[test]
    fn debounce_uses_a_quiet_period_without_sleeping() {
        let start = Instant::now();
        let mut debounce = Debouncer::default();
        debounce.record(start);
        debounce.record(start + Duration::from_millis(150));
        assert!(!debounce.take_ready(start + Duration::from_millis(349)));
        assert!(debounce.take_ready(start + Duration::from_millis(350)));
        assert!(!debounce.take_ready(start + Duration::from_secs(1)));
    }

    #[test]
    fn filesystem_bursts_start_one_scan_after_200_ms() {
        let start = Instant::now();
        let worker = FakeWorker::new([]);
        let mut coordinator = RefreshCoordinator::<usize, _>::new(worker);
        coordinator.filesystem_event(start);
        coordinator.filesystem_event(start + Duration::from_millis(100));
        coordinator.poll(start + Duration::from_millis(299));
        assert!(!coordinator.is_scanning());
        coordinator.poll(start + Duration::from_millis(300));
        assert!(coordinator.is_scanning());
        assert_eq!(statuses(&mut coordinator), vec![RefreshStatus::Refreshing]);
    }

    #[test]
    fn manual_refresh_bypasses_debounce() {
        let start = Instant::now();
        let worker = FakeWorker::new([]);
        let mut coordinator = RefreshCoordinator::<usize, _>::new(worker);
        coordinator.filesystem_event(start);
        coordinator.manual_refresh();
        assert!(coordinator.is_scanning());
        assert!(!coordinator.has_pending_refresh());
    }

    #[test]
    fn event_during_scan_schedules_exactly_one_follow_up() {
        let start = Instant::now();
        let worker = FakeWorker::new([Ok(1), Ok(2)]);
        let mut coordinator = RefreshCoordinator::new(worker);
        coordinator.manual_refresh();
        coordinator.filesystem_event(start);
        coordinator.filesystem_event(start + Duration::from_millis(10));
        coordinator.manual_refresh();
        assert!(coordinator.has_pending_refresh());

        coordinator.poll(start);
        assert!(coordinator.is_scanning());
        assert!(!coordinator.has_pending_refresh());
        assert_eq!(coordinator.worker.starts, 2);

        coordinator.poll(start);
        assert!(!coordinator.is_scanning());
        assert_eq!(coordinator.worker.starts, 2);
    }

    #[test]
    fn failure_keeps_snapshot_absent_and_recovery_publishes_next_snapshot() {
        let start = Instant::now();
        let worker = FakeWorker::new([
            Err(ScanError::Failed("permission denied".into())),
            Ok("fresh"),
        ]);
        let mut coordinator = RefreshCoordinator::new(worker);
        coordinator.manual_refresh();
        coordinator.poll(start);
        assert_eq!(
            statuses(&mut coordinator),
            vec![
                RefreshStatus::Refreshing,
                RefreshStatus::RefreshError("permission denied".into())
            ]
        );

        coordinator.manual_refresh();
        coordinator.poll(start);
        assert_eq!(
            coordinator.drain_events().collect::<Vec<_>>(),
            vec![
                RefreshEvent::Status(RefreshStatus::Refreshing),
                RefreshEvent::Snapshot("fresh"),
                RefreshEvent::Status(RefreshStatus::UpdatedJustNow)
            ]
        );
    }

    #[test]
    fn updated_status_returns_to_up_to_date_deterministically() {
        let start = Instant::now();
        let worker = FakeWorker::new([Ok(1)]);
        let mut coordinator = RefreshCoordinator::new(worker);
        coordinator.manual_refresh();
        coordinator.poll(start);
        coordinator.drain_events().for_each(drop);
        coordinator.poll(
            (start + UPDATED_STATUS_DURATION)
                .checked_sub(Duration::from_millis(1))
                .unwrap(),
        );
        assert!(coordinator.drain_events().next().is_none());
        coordinator.poll(start + UPDATED_STATUS_DURATION);
        assert_eq!(statuses(&mut coordinator), vec![RefreshStatus::UpToDate]);
    }

    #[test]
    fn target_missing_and_watcher_error_have_actionable_statuses() {
        let start = Instant::now();
        let worker = FakeWorker::<usize>::new([Err(ScanError::TargetMissing)]);
        let mut coordinator = RefreshCoordinator::new(worker);
        coordinator.watcher_error("backend stopped");
        coordinator.manual_refresh();
        coordinator.poll(start);
        assert_eq!(
            statuses(&mut coordinator),
            vec![
                RefreshStatus::WatcherError("backend stopped".into()),
                RefreshStatus::Refreshing,
                RefreshStatus::TargetMissing
            ]
        );
        assert!(
            RefreshStatus::WatcherError("oops".into())
                .to_string()
                .contains("press r")
        );
    }

    #[test]
    fn shutdown_ignores_late_results_and_requests() {
        let now = Instant::now();
        let worker = FakeWorker::new([Ok(7)]);
        let mut coordinator = RefreshCoordinator::new(worker);
        coordinator.manual_refresh();
        coordinator.shutdown();
        coordinator.filesystem_event(now);
        coordinator.manual_refresh();
        coordinator.poll(now + Duration::from_secs(10));
        assert_eq!(
            coordinator.drain_events().collect::<Vec<_>>(),
            vec![RefreshEvent::Status(RefreshStatus::ShuttingDown)]
        );
        assert!(coordinator.worker.shutdown);
    }

    #[test]
    fn bounded_callback_queue_turns_overflow_into_rescan() {
        let (sender, receiver) = bounded(1);
        let overflowed = AtomicBool::new(false);
        let event = Event::new(EventKind::Create(CreateKind::File)).add_path("one.md".into());
        enqueue_notify_result(Ok(event.clone()), &sender, &overflowed);
        enqueue_notify_result(Ok(event), &sender, &overflowed);
        assert!(overflowed.load(Ordering::Acquire));
        assert!(matches!(
            receive_watch_event(&receiver, &overflowed),
            Ok(WatchEvent::Changed(paths)) if paths == vec![PathBuf::from("one.md")]
        ));
        assert_eq!(
            receive_watch_event(&receiver, &overflowed),
            Ok(WatchEvent::Rescan)
        );
        assert_eq!(
            receive_watch_event(&receiver, &overflowed),
            Err(TryRecvError::Empty)
        );
    }

    #[test]
    fn notify_rescan_indications_force_full_scan_messages() {
        let (sender, receiver) = bounded(2);
        let overflowed = AtomicBool::new(false);
        enqueue_notify_result(Ok(Event::new(EventKind::Other)), &sender, &overflowed);
        assert_eq!(
            receive_watch_event(&receiver, &overflowed),
            Ok(WatchEvent::Rescan)
        );
    }

    #[test]
    fn native_watcher_reports_create_modify_rename_and_remove() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let watcher = NotifyWatcher::new(directory.path()).expect("watch directory");
        let first = directory.path().join("first.md");
        let second = directory.path().join("second.md");

        fs::write(&first, "- [ ] one\n").expect("create");
        wait_for_path(&watcher, &first);
        fs::write(&first, "- [x] one\n").expect("modify");
        wait_for_path(&watcher, &first);
        fs::rename(&first, &second).expect("rename");
        wait_for_path(&watcher, &second);
        fs::remove_file(&second).expect("remove");
        wait_for_path(&watcher, &second);
    }

    fn wait_for_path(watcher: &NotifyWatcher, expected: &Path) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            match watcher.try_recv() {
                Ok(WatchEvent::Changed(paths) | WatchEvent::ChangedDirectories(paths))
                    if paths.iter().any(|path| {
                        path == expected
                            || path.file_name().is_some_and(|name| {
                                expected
                                    .file_name()
                                    .is_some_and(|expected| name == expected)
                            })
                    }) =>
                {
                    return;
                }
                Ok(_) | Err(TryRecvError::Empty) => thread::yield_now(),
                Err(TryRecvError::Disconnected) => panic!("native watcher disconnected"),
            }
        }
        panic!(
            "timed out waiting for native event for {} at {:?}",
            expected.display(),
            SystemTime::now()
        );
    }

    #[test]
    fn thread_worker_does_not_scan_in_requesting_thread() {
        let caller = thread::current().id();
        let scanned_on = Arc::new(Mutex::new(None));
        let worker_scanned_on = Arc::clone(&scanned_on);
        let mut worker = ThreadScanWorker::new(move || {
            *worker_scanned_on.lock().expect("mutex") = Some(thread::current().id());
            Ok::<_, ScanError>(())
        });
        worker.try_start().expect("start");
        let result = worker
            .results
            .recv_timeout(Duration::from_secs(2))
            .expect("worker result");
        assert!(result.is_ok());
        assert_ne!(*scanned_on.lock().expect("mutex"), Some(caller));
        worker.shutdown();
    }
}
