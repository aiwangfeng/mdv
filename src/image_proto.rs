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
const MAX_IMAGE_CACHE: usize = 64;

pub struct ImageManager {
    picker: Option<Picker>,
    pub cache: HashMap<String, StatefulProtocol>,
    cache_order: VecDeque<String>,
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
            .finish_non_exhaustive()
    }
}

impl ImageManager {
    pub fn new(no_images: bool) -> Self {
        let (sender, receiver) = mpsc::channel();
        let picker = if no_images {
            None
        } else {
            Picker::from_query_stdio().ok()
        };
        Self {
            picker,
            cache: HashMap::new(),
            cache_order: VecDeque::new(),
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

    pub fn cache_key(src: &str, base_dir: &Path) -> String {
        if src.starts_with('/') || src.starts_with("~/") || src.starts_with("file://") {
            src.to_string()
        } else {
            base_dir.join(src).to_string_lossy().into_owned()
        }
    }

    fn resolve_safe_path(src: &str, base_dir: &Path) -> Option<PathBuf> {
        if src.starts_with("http://") || src.starts_with("https://") || src.starts_with("data:") {
            return None;
        }

        let candidate = if let Some(rest) = src.strip_prefix("~/") {
            let home = dirs_home()?;
            home.join(rest)
        } else if let Some(stripped) = src.strip_prefix("file://") {
            PathBuf::from(stripped)
        } else if src.starts_with('/') {
            PathBuf::from(src)
        } else {
            base_dir.join(src)
        };

        let canonical = candidate.canonicalize().ok()?;
        if !canonical.is_file() {
            return None;
        }

        let path_str = canonical.to_string_lossy();
        if path_str.starts_with("/etc")
            || path_str.starts_with("/private/etc")
            || path_str.starts_with("/proc")
            || path_str.starts_with("/sys")
            || path_str.starts_with("/dev")
        {
            log::warn!("Rejected image path in restricted system directory: {src}");
            return None;
        }

        Some(canonical)
    }

    /// Dispatch a background thread to load an image.
    pub fn load_async(&mut self, src: &str, base_dir: &Path) {
        if self.picker.is_none() {
            return;
        }
        let key = Self::cache_key(src, base_dir);
        if self.cache.contains_key(&key)
            || self.pending.contains(&key)
            || self.queued.contains(&key)
            || self.failed.contains(&key)
        {
            return;
        }

        let Some(path) = Self::resolve_safe_path(src, base_dir) else {
            self.failed.insert(key);
            return;
        };

        if !self.start_load(key.clone(), path.clone()) {
            self.queued.insert(key.clone());
            self.queue.push_back((key, path));
        }
    }

    /// Process received images from background threads.
    pub fn process_incoming(&mut self) -> bool {
        let mut new_images = false;
        if self.picker.is_none() {
            return false;
        }
        while let Ok((key, result)) = self.receiver.try_recv() {
            self.pending.remove(&key);
            match result {
                Ok(img) => {
                    let protocol = self.picker.as_mut().unwrap().new_resize_protocol(img);
                    self.cache.insert(key.clone(), protocol);
                    self.cache_order.retain(|k| k != &key);
                    self.cache_order.push_back(key);
                    while self.cache.len() > MAX_IMAGE_CACHE {
                        if let Some(old) = self.cache_order.pop_front() {
                            self.cache.remove(&old);
                        }
                    }
                    new_images = true;
                }
                Err(e) => {
                    log::warn!("Failed to load image '{key}': {e}");
                    self.failed.insert(key);
                }
            }
        }
        self.dispatch_queued_loads();
        new_images
    }

    pub fn get_protocol_mut(
        &mut self,
        src: &str,
        base_dir: &Path,
    ) -> Option<&mut StatefulProtocol> {
        let key = Self::cache_key(src, base_dir);
        if self.cache.contains_key(&key) {
            self.cache_order.retain(|k| k != &key);
            self.cache_order.push_back(key.clone());
        }
        self.cache.get_mut(&key)
    }

    fn start_load(&mut self, src: String, path: PathBuf) -> bool {
        let prev = self.active_loads.fetch_add(1, Ordering::Relaxed);
        if prev >= MAX_CONCURRENT_LOADS {
            self.active_loads.fetch_sub(1, Ordering::Relaxed);
            return false;
        }

        let sender = self.sender.clone();
        let active_loads = Arc::clone(&self.active_loads);
        self.pending.insert(src.clone());

        thread::spawn(move || {
            let result = image::open(&path);
            active_loads.fetch_sub(1, Ordering::Relaxed);
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

fn dirs_home() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|base| base.home_dir().to_path_buf())
}
