// src/image_proto.rs
// Wraps ratatui-image: protocol detection, image loading, and stateful widget state.

use image::DynamicImage;
use ratatui_image::{picker::Picker, protocol::StatefulProtocol};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

const MAX_CONCURRENT_LOADS: usize = 4;

pub struct ImageManager {
    picker: Option<Picker>,
    pub cache: HashMap<String, StatefulProtocol>,
    pending: HashSet<String>,
    queued: HashSet<String>,
    queue: VecDeque<(String, PathBuf)>,
    failed: HashSet<String>,
    receiver: mpsc::Receiver<(String, Result<DynamicImage, image::ImageError>)>,
    sender: mpsc::Sender<(String, Result<DynamicImage, image::ImageError>)>,
    active_loads: Arc<AtomicUsize>,
}

impl std::fmt::Debug for ImageManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageManager")
            .field("has_picker", &self.picker.is_some())
            .field("cached_images", &self.cache.len())
            .field("pending_images", &self.pending.len())
            .field("queued_images", &self.queue.len())
            .field("failed_images", &self.failed.len())
            .finish()
    }
}

impl ImageManager {
    pub fn new(no_images: bool) -> Self {
        let (sender, receiver) = mpsc::channel();
        if no_images {
            return Self {
                picker: None,
                cache: HashMap::new(),
                pending: HashSet::new(),
                queued: HashSet::new(),
                queue: VecDeque::new(),
                failed: HashSet::new(),
                sender,
                receiver,
                active_loads: Arc::new(AtomicUsize::new(0)),
            };
        }
        let picker = Picker::from_query_stdio().ok();
        Self {
            picker,
            cache: HashMap::new(),
            pending: HashSet::new(),
            queued: HashSet::new(),
            queue: VecDeque::new(),
            failed: HashSet::new(),
            sender,
            receiver,
            active_loads: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.picker.is_some()
    }

    fn resolve_safe_path(src: &str, base_dir: &Path) -> Option<PathBuf> {
        let path = if src.starts_with('/') {
            let abs = PathBuf::from(src);
            let canonical = abs.canonicalize().ok()?;
            let allowed_prefixes = [std::env::var("HOME").ok()?, std::env::var("PWD").ok()?];
            let allowed = allowed_prefixes
                .iter()
                .filter_map(|p| Path::new(p).canonicalize().ok())
                .any(|p| canonical.starts_with(p));
            if !allowed {
                log::warn!(
                    "Rejected absolute path outside allowed directories: {}",
                    src
                );
                return None;
            }
            abs
        } else if let Some(rest) = src.strip_prefix("~/") {
            let home = dirs_home();
            let resolved = home.join(rest);
            let canonical = resolved.canonicalize().ok()?;
            let home_canonical = home.canonicalize().ok()?;
            if !canonical.starts_with(&home_canonical) {
                log::warn!("Rejected path outside home directory: {}", src);
                return None;
            }
            resolved
        } else {
            let resolved = base_dir.join(src);
            let canonical = resolved.canonicalize().ok()?;
            let base_canonical = base_dir.canonicalize().ok()?;
            if !canonical.starts_with(&base_canonical) {
                log::warn!("Rejected path outside base directory: {}", src);
                return None;
            }
            resolved
        };
        Some(path)
    }

    /// Dispatch a background thread to load an image.
    pub fn load_async(&mut self, src: &str, base_dir: &Path) {
        if self.picker.is_none() {
            return;
        }
        if self.cache.contains_key(src)
            || self.pending.contains(src)
            || self.queued.contains(src)
            || self.failed.contains(src)
        {
            return;
        }

        let Some(path) = Self::resolve_safe_path(src, base_dir) else {
            self.failed.insert(src.to_string());
            return;
        };

        let src_owned = src.to_string();
        if !self.start_load(src_owned.clone(), path.clone()) {
            self.queued.insert(src_owned.clone());
            self.queue.push_back((src_owned, path));
        }
    }

    /// Process received images from background threads.
    pub fn process_incoming(&mut self) -> bool {
        let mut new_images = false;
        if self.picker.is_none() {
            return false;
        }
        while let Ok((src, result)) = self.receiver.try_recv() {
            self.pending.remove(&src);
            match result {
                Ok(img) => {
                    let protocol = self.picker.as_mut().unwrap().new_resize_protocol(img);
                    self.cache.insert(src, protocol);
                    new_images = true;
                }
                Err(e) => {
                    log::warn!("Failed to load image '{}': {}", src, e);
                    self.failed.insert(src);
                }
            }
        }
        self.dispatch_queued_loads();
        new_images
    }

    pub fn get_protocol_mut(&mut self, src: &str) -> Option<&mut StatefulProtocol> {
        self.cache.get_mut(src)
    }

    fn start_load(&mut self, src: String, path: PathBuf) -> bool {
        loop {
            let current = self.active_loads.load(Ordering::Relaxed);
            if current >= MAX_CONCURRENT_LOADS {
                return false;
            }

            if self
                .active_loads
                .compare_exchange(current, current + 1, Ordering::SeqCst, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }

        let sender = self.sender.clone();
        let active_loads = Arc::clone(&self.active_loads);
        self.pending.insert(src.clone());

        thread::spawn(move || {
            let result = image::open(&path);
            active_loads.fetch_sub(1, Ordering::SeqCst);
            if sender.send((src, result)).is_err() {
                log::warn!("Failed to send loaded image - receiver dropped");
            }
        });

        true
    }

    fn dispatch_queued_loads(&mut self) {
        while let Some((src, path)) = self.queue.pop_front() {
            self.queued.remove(&src);
            if self.cache.contains_key(&src)
                || self.pending.contains(&src)
                || self.failed.contains(&src)
            {
                continue;
            }
            if !self.start_load(src.clone(), path.clone()) {
                self.queued.insert(src.clone());
                self.queue.push_front((src, path));
                break;
            }
        }
    }
}

fn dirs_home() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}
