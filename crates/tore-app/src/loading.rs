//! Flight preparation without holding the window event callback.
//! Behaviour: docs/spec/loading-screen.md.
use std::{
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Instant,
};

// CPU weights measured on the private UKR/F18 profile probe (2026-10-16).
// Graphics is a fitted 500 ms reservation until measured on a GPU host.
const WEIGHTS: [u32; 4] = [2442, 11, 164, 500];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stage {
    Content,
    Connecting,
    Waiting,
    Replay,
    Mission,
    Aircraft,
    Scenery,
    Graphics,
    Ready,
}
impl Stage {
    pub fn words(self) -> &'static str {
        match self {
            Self::Content => "Checking game content",
            Self::Connecting => "Connecting to the host",
            Self::Waiting => "Waiting for the host",
            Self::Replay => "Loading the replay",
            Self::Mission => "Building the mission",
            Self::Aircraft => "Loading aircraft",
            Self::Scenery => "Loading terrain and scenery",
            Self::Graphics => "Preparing graphics",
            Self::Ready => "Starting flight",
        }
    }
    #[cfg(test)]
    pub fn percent(self) -> u32 {
        self.completed_weight() * 100 / WEIGHTS.iter().sum::<u32>()
    }
    fn completed_weight(self) -> u32 {
        WEIGHTS[..self.index()].iter().sum()
    }
    fn index(self) -> usize {
        match self {
            Self::Content | Self::Connecting | Self::Waiting | Self::Replay | Self::Mission => 0,
            Self::Aircraft => 1,
            Self::Scenery => 2,
            Self::Graphics => 3,
            Self::Ready => 4,
        }
    }
    fn estimate(self, elapsed: f64) -> f64 {
        if self == Self::Ready {
            return 100.;
        }
        if matches!(self, Self::Content | Self::Connecting | Self::Waiting) {
            return 0.;
        }
        let weight = f64::from(WEIGHTS[self.index()]);
        let fraction = elapsed.max(0.) / (elapsed.max(0.) + weight / 1000.);
        (f64::from(self.completed_weight()) + weight * fraction) * 100.
            / f64::from(WEIGHTS.iter().sum::<u32>())
    }
}
#[derive(Clone)]
pub(crate) struct Progress(Arc<Mutex<(Stage, Instant)>>);
impl Progress {
    fn new() -> Self {
        Self(Arc::new(Mutex::new((Stage::Mission, Instant::now()))))
    }
    pub fn stage(&self, stage: Stage) {
        let mut current = self.0.lock().unwrap_or_else(|e| e.into_inner());
        log::info!(
            "loading: {:?} {:.3} ms",
            current.0,
            current.1.elapsed().as_secs_f64() * 1000.
        );
        #[cfg(test)]
        eprintln!(
            "loading stage {:?}: {:.3} ms",
            current.0,
            current.1.elapsed().as_secs_f64() * 1000.
        );
        *current = (stage, Instant::now());
    }
    fn current(&self) -> (Stage, f64) {
        let current = self.0.lock().unwrap_or_else(|e| e.into_inner());
        (current.0, current.1.elapsed().as_secs_f64())
    }
}
pub(crate) struct Job<T> {
    receive: mpsc::Receiver<Result<T, String>>,
    pub progress: Progress,
    photo: usize,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl<T: Send + 'static> Job<T> {
    pub fn spawn(
        work: impl FnOnce(Progress) -> Result<T, String> + Send + 'static,
    ) -> Result<Self, String> {
        static PHOTO: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let (send, receive) = mpsc::channel();
        let progress = Progress::new();
        let status = progress.clone();
        let thread = std::thread::Builder::new()
            .name("flight-loader".into())
            .spawn(move || {
                // Decode before mission work, on this thread, not the window thread.
                prepare_photos();
                let result = work(status.clone());
                // A failed load is not completion: leave the last real stage up.
                if result.is_ok() {
                    status.stage(Stage::Ready);
                }
                let _ = send.send(result);
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            receive,
            progress,
            photo: PHOTO.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % 2,
            thread: Some(thread),
        })
    }
    /// The worker, still running or finished. Dropping the job discards its
    /// result; the caller joins this handle after the window closes.
    pub fn into_thread(mut self) -> Option<std::thread::JoinHandle<()>> {
        self.thread.take()
    }
    pub fn poll(&self) -> Option<Result<T, String>> {
        match self.receive.try_recv() {
            Ok(value) => Some(value),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => {
                Some(Err("The flight loader stopped unexpectedly.".into()))
            }
        }
    }
    pub fn draw(&self, pixels: &mut Vec<u8>, size: [u32; 2], details: [&str; 2]) -> [u32; 2] {
        let (stage, elapsed) = self.progress.current();
        draw_window(pixels, size, self.photo, stage, elapsed, details)
    }
}
struct Rgb {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}
struct Photos {
    frames: [Option<Rgb>; 2],
    #[cfg(test)]
    decode_ms: f64,
}
fn bundled_jpegs() -> [(&'static str, &'static [u8]); 2] {
    [
        (
            "harrier.jpg",
            include_bytes!("../assets/loading/harrier.jpg"),
        ),
        (
            "thunderbirds.jpg",
            include_bytes!("../assets/loading/thunderbirds.jpg"),
        ),
    ]
}
fn decode_jpeg(bytes: &[u8]) -> Result<Rgb, String> {
    use zune_jpeg::JpegDecoder;
    use zune_jpeg::zune_core::bytestream::ZCursor;
    use zune_jpeg::zune_core::colorspace::ColorSpace;
    use zune_jpeg::zune_core::options::DecoderOptions;
    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGB);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(bytes), options);
    let pixels = decoder.decode().map_err(|error| error.to_string())?;
    let info = decoder
        .info()
        .ok_or_else(|| "jpeg has no dimensions".to_string())?;
    let width = usize::from(info.width);
    let height = usize::from(info.height);
    if width == 0 || height == 0 || pixels.len() != width * height * 3 {
        return Err(format!(
            "jpeg decoded to {width}x{height}, {} bytes",
            pixels.len()
        ));
    }
    Ok(Rgb {
        width,
        height,
        pixels,
    })
}
fn load_photos() -> Photos {
    let started = Instant::now();
    let mut frames = [None, None];
    for (index, (name, bytes)) in bundled_jpegs().into_iter().enumerate() {
        match decode_jpeg(bytes) {
            Ok(photo) => frames[index] = Some(photo),
            Err(error) => log::error!("loading: {name} decode failed: {error}"),
        }
    }
    let decode_ms = started.elapsed().as_secs_f64() * 1000.;
    let describe = |photo: &Option<Rgb>| match photo {
        Some(photo) => format!("{}x{}", photo.width, photo.height),
        None => "failed".to_string(),
    };
    log::info!(
        "loading: jpeg decode {decode_ms:.3} ms on {} ({}, {})",
        std::thread::current().name().unwrap_or("unnamed"),
        describe(&frames[0]),
        describe(&frames[1]),
    );
    Photos {
        frames,
        #[cfg(test)]
        decode_ms,
    }
}
static PHOTOS: OnceLock<Photos> = OnceLock::new();
static DECODE_STARTED: AtomicBool = AtomicBool::new(false);
fn cached_photos() -> Option<&'static Photos> {
    PHOTOS.get()
}
/// Decode both photographs on this thread. The flight loader calls this
/// before mission work. The headless snapshot calls it because that path
/// has no window. The window thread must not.
pub(crate) fn prepare_photos() {
    DECODE_STARTED.store(true, Ordering::Release);
    let _ = PHOTOS.get_or_init(load_photos);
}
fn request_background_decode() {
    if PHOTOS.get().is_some() || DECODE_STARTED.load(Ordering::Acquire) {
        return;
    }
    if DECODE_STARTED.swap(true, Ordering::AcqRel) {
        return;
    }
    if let Err(error) = std::thread::Builder::new()
        .name("loading-photos".into())
        .spawn(|| {
            let _ = PHOTOS.get_or_init(load_photos);
        })
    {
        DECODE_STARTED.store(false, Ordering::Release);
        log::error!("loading: jpeg decode thread failed: {error}");
    }
}
/// `loading`, `loading-thunderbirds`, or either name with a `-WIDTHxHEIGHT`
/// window. The battery states stay 640 by 480.
pub(crate) fn snapshot_window(state: &str) -> Option<(usize, [u32; 2])> {
    let rest = state.strip_prefix("loading")?;
    let (label, window) = match rest.rsplit_once('-') {
        Some((label, size)) => match parse_window(size) {
            Some(window) => (label, window),
            None => (rest, [640, 480]),
        },
        None => (rest, [640, 480]),
    };
    Some((usize::from(label.contains("thunderbirds")), window))
}
fn parse_window(size: &str) -> Option<[u32; 2]> {
    let (width, height) = size.split_once('x')?;
    let width = width.parse::<u32>().ok()?;
    let height = height.parse::<u32>().ok()?;
    ((320..=7680).contains(&width) && (240..=4320).contains(&height)).then_some([width, height])
}
fn blit_photo(pixels: &mut [u8], width: usize, height: usize, photo: Option<&Rgb>) {
    let Some(photo) = photo else {
        pixels.fill(0);
        for pixel in pixels.chunks_exact_mut(4) {
            pixel[3] = 255;
        }
        return;
    };
    let (w, h) = (photo.width, photo.height);
    let scale = (width as f64 / w as f64).max(height as f64 / h as f64);
    for y in 0..height {
        for x in 0..width {
            let sx = ((x as f64 - width as f64 / 2.) / scale + w as f64 / 2.)
                .clamp(0., (w - 1) as f64) as usize;
            let sy = ((y as f64 - height as f64 / 2.) / scale + h as f64 / 2.)
                .clamp(0., (h - 1) as f64) as usize;
            let shade = 0.85 - 0.55 * y as f64 / height as f64;
            let dst = (y * width + x) * 4;
            let src = (sy * w + sx) * 3;
            for c in 0..3 {
                pixels[dst + c] = (photo.pixels[src + c] as f64 * shade) as u8;
            }
            pixels[dst + 3] = 255;
        }
    }
}
fn dimensions(bytes: &[u8]) -> (usize, usize) {
    (
        u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize,
        u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize,
    )
}
/// Keep the photograph's aspect ratio and cover the whole window. Text stays
/// in a centered 640x480 logical area, matching the bundled menu font metrics.
pub(crate) fn draw_window(
    pixels: &mut Vec<u8>,
    window: [u32; 2],
    photo: usize,
    stage: Stage,
    elapsed: f64,
    details: [&str; 2],
) -> [u32; 2] {
    let ratio = (window[0].max(1) as f64 / window[1].max(1) as f64).clamp(0.25, 6.);
    let size = if ratio >= 4. / 3. {
        [(480. * ratio).round() as u32, 480]
    } else {
        [640, (640. / ratio).round() as u32]
    };
    pixels.resize(size[0] as usize * size[1] as usize * 4, 0);
    // The window thread only samples. A loader that has not finished yet
    // leaves a dark field until the next redraw.
    if cached_photos().is_none() {
        request_background_decode();
    }
    draw_sized(pixels, size, photo, stage, elapsed, details);
    size
}
fn draw_sized(
    pixels: &mut [u8],
    size: [u32; 2],
    photo: usize,
    stage: Stage,
    elapsed: f64,
    details: [&str; 2],
) {
    use crate::menu::{Canvas, HEIGHT, WIDTH};
    let (width, height) = (size[0] as usize, size[1] as usize);
    let decoded = cached_photos().and_then(|photos| photos.frames[photo & 1].as_ref());
    blit_photo(pixels, width, height, decoded);
    let left = (width - WIDTH) / 2;
    let top = (height - HEIGHT) / 2;
    let mut overlay = vec![0; WIDTH * HEIGHT * 4];
    for y in 0..HEIGHT {
        overlay[y * WIDTH * 4..(y + 1) * WIDTH * 4].copy_from_slice(
            &pixels[((y + top) * width + left) * 4..((y + top) * width + left + WIDTH) * 4],
        );
    }
    let output = pixels;
    let pixels = &mut overlay[..];
    let logo = include_bytes!("../assets/loading/logo.rgba");
    let (w, h) = dimensions(logo);
    for y in 0..h {
        for x in 0..w {
            let src = 8 + (y * w + x) * 4;
            let dst = ((y + 55) * WIDTH + x + (WIDTH - w) / 2) * 4;
            let a = logo[src + 3] as u32;
            for c in 0..3 {
                pixels[dst + c] =
                    ((logo[src + c] as u32 * a + pixels[dst + c] as u32 * (255 - a)) / 255) as u8;
            }
        }
    }
    let mut canvas = Canvas(pixels);
    let font = crate::menu::flat_font_large([245, 245, 245]);
    canvas.centered_text(&font, stage.words(), (40, 330, 560, 24));
    canvas.rect((80, 370, 480, 16), [30, 35, 40, 255]);
    canvas.rect(
        (80, 370, (4.8 * stage.estimate(elapsed)) as i32, 16),
        [180, 200, 220, 255],
    );
    // The moving highlight continues even when estimates round to the same value.
    let x = 80 + ((elapsed * 100.) as i32 % 460);
    canvas.rect((x, 390, 20, 2), [220, 230, 240, 255]);
    if matches!(stage, Stage::Content | Stage::Connecting | Stage::Waiting) {
        canvas.rect((x, 370, 20, 16), [180, 200, 220, 255]);
        canvas.centered_text(&font, &format!("{elapsed:.0}s"), (80, 405, 480, 24));
    } else {
        canvas.centered_text(
            &font,
            &format!(
                "Estimated {:.1}%   {:.0}s",
                stage.estimate(elapsed),
                elapsed
            ),
            (80, 405, 480, 24),
        );
    }
    canvas.centered_text(&font, details[0], (20, 285, 600, 18));
    canvas.centered_text(&font, details[1], (20, 307, 600, 18));
    // End the temporary borrow before copying the center back.
    for y in 0..HEIGHT {
        output[((y + top) * width + left) * 4..((y + top) * width + left + WIDTH) * 4]
            .copy_from_slice(&overlay[y * WIDTH * 4..(y + 1) * WIDTH * 4]);
    }
}

pub(crate) fn prepare_mission(
    spec: &crate::mission::MissionSpec,
    resources: &std::collections::BTreeMap<String, Vec<u8>>,
    player: Arc<tore_world::aircraft_type::AircraftType>,
    progress: &Progress,
) -> Result<
    (
        crate::world::Built,
        crate::combat_view::CombatView,
        crate::scenery::Scenery,
    ),
    String,
> {
    let mut models = Vec::new();
    let mut load = |id| -> tore_world::WorldResult<Arc<tore_world::aircraft_type::AircraftType>> {
        let model = crate::aircraft::Airframe::load(resources, id)?;
        let kind = Arc::clone(&model.kind);
        models.push(model);
        Ok(kind)
    };
    let font = crate::ordnance::label_font();
    let label = crate::ordnance::weapon_label(resources, &font);
    let built = crate::world::World::build(
        spec,
        resources,
        crate::world::Seating::SinglePlayer,
        &mut crate::world::Hooks {
            player: Some(player),
            load: Some(&mut load),
            weapon_label: Some(&label),
            ground_variation: None,
        },
    )
    .map_err(|e| e.to_string())?;
    progress.stage(Stage::Aircraft);
    let view = crate::combat_view::CombatView::with_models(
        &built.world.combat,
        built.world.picture_plane().0,
        resources,
        models,
    )
    .map_err(|e| e.to_string())?;
    progress.stage(Stage::Scenery);
    let scenery = crate::scenery::Scenery::build(resources, &built.world.terrain)
        .map_err(|e| e.to_string())?;
    progress.stage(Stage::Graphics);
    Ok((built, view, scenery))
}

pub(crate) type NetParts = (
    crate::world::World,
    crate::flight_map::Plan,
    crate::aircraft::Airframe,
    crate::combat_view::CombatView,
    crate::scenery::Scenery,
    Option<crate::renderer::LoadingGraphics>,
);

pub(crate) type SelectionParts = (
    crate::world::World,
    crate::aircraft::Airframe,
    crate::combat_view::CombatView,
    crate::scenery::Scenery,
    Option<crate::renderer::LoadingGraphics>,
);

pub(crate) type MissionParts = (
    crate::world::Built,
    crate::combat_view::CombatView,
    crate::scenery::Scenery,
    Option<crate::renderer::LoadingGraphics>,
    Option<crate::replay::recorder::Recorder>,
);

pub(crate) enum Prepared {
    NetRebuilt(Box<crate::net::session::Built>),
    Selection(Box<SelectionParts>),
    Restart(
        Box<(
            crate::world::World,
            crate::world::Restarted,
            Option<crate::replay::recorder::Recorder>,
        )>,
    ),
    Net(Box<NetParts>),
    Mission(Box<MissionParts>),
    Replay(
        Box<crate::replay::viewer::Viewer>,
        Option<Box<crate::renderer::LoadingGraphics>>,
    ),
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn creator_smoke_restart_keeps_the_accepted_start() {
        let resources = tore_world::test_support::resources::resources();
        let id = tore_formats::aircraft::AircraftId::F18;
        let spec = crate::mission::MissionSpec::new("UKR", id);
        let aircraft = tore_world::aircraft_type::load_type(&resources, id).unwrap();
        let mut world =
            crate::world::World::new(&spec, &resources, crate::world::Seating::SinglePlayer)
                .unwrap();
        verify_restart(&mut world, &aircraft, &resources).unwrap();
        world.cockpits[crate::OWN].flight.fuel -= 100.;
        assert!(verify_restart(&mut world, &aircraft, &resources).is_err());
    }
    #[test]
    fn stage_progress_is_monotonic_and_complete() {
        let stages = [
            Stage::Mission,
            Stage::Aircraft,
            Stage::Scenery,
            Stage::Graphics,
            Stage::Ready,
        ];
        assert_eq!(stages[0].percent(), 0);
        assert_eq!(stages[4].percent(), 100);
        assert!(
            stages
                .windows(2)
                .all(|w| w[0].estimate(0.) < w[1].estimate(0.))
        );
    }
    #[test]
    fn estimated_progress_uses_weights_without_claiming_completion() {
        assert_eq!(WEIGHTS.iter().sum::<u32>(), 3117);
        for stage in [
            Stage::Mission,
            Stage::Aircraft,
            Stage::Scenery,
            Stage::Graphics,
        ] {
            assert!(stage.estimate(1.) < stage.estimate(10.));
            assert!(stage.estimate(1000.) < 100.);
        }
        assert_eq!(Stage::Ready.estimate(0.), 100.);
    }
    #[test]
    fn loading_photo_covers_wide_and_tall_windows() {
        prepare_photos();
        let mut pixels = Vec::new();
        for size in [[1920, 1080], [800, 1200]] {
            let drawn = draw_window(
                &mut pixels,
                size,
                0,
                Stage::Scenery,
                1.,
                ["Host", "Players"],
            );
            assert!(
                (drawn[0] as f64 / drawn[1] as f64 - size[0] as f64 / size[1] as f64).abs() < 0.003
            );
            assert!(pixels.chunks_exact(4).all(|p| p[3] == 255));
            assert!(pixels.chunks_exact(4).any(|p| p[..3] != [0, 0, 0]));
        }
    }
    #[test]
    fn a_missing_photo_is_a_plain_dark_background() {
        let mut pixels = vec![7; 2 * 2 * 4];
        blit_photo(&mut pixels, 2, 2, None);
        assert!(pixels.chunks_exact(4).all(|p| p == [0, 0, 0, 255]));
    }
    #[test]
    fn a_bad_jpeg_is_rejected() {
        assert!(decode_jpeg(b"not a jpeg").is_err());
    }
    #[test]
    fn flight_loader_decodes_the_bundled_jpegs() {
        std::thread::Builder::new()
            .name("flight-loader".into())
            .spawn(prepare_photos)
            .unwrap()
            .join()
            .unwrap();
        let photos = cached_photos().expect("decoded photographs");
        eprintln!("loading jpeg decode {:.3} ms", photos.decode_ms);
        for frame in &photos.frames {
            let frame = frame.as_ref().expect("bundled jpeg");
            assert_eq!((frame.width, frame.height), (2560, 1707));
            assert_eq!(frame.pixels.len(), 2560 * 1707 * 3);
        }
    }
    #[test]
    fn pending_work_never_waits_for_worker() {
        let (send, receive) = mpsc::channel();
        let job = Job::spawn(move |_| {
            receive.recv().unwrap();
            Ok(42)
        })
        .unwrap();
        for _ in 0..1000 {
            assert!(job.poll().is_none());
        }
        send.send(()).unwrap();
    }
    #[test]
    fn loading_patch_has_valid_dimensions() {
        let logo = include_bytes!("../assets/loading/logo.rgba").as_slice();
        let (w, h) = dimensions(logo);
        assert_eq!(logo.len(), 8 + w * h * 4);
    }
    #[test]
    fn loading_snapshot_states_keep_the_battery_size() {
        assert_eq!(snapshot_window("loading"), Some((0, [640, 480])));
        assert_eq!(
            snapshot_window("loading-thunderbirds"),
            Some((1, [640, 480]))
        );
        assert_eq!(
            snapshot_window("loading-1920x1080"),
            Some((0, [1920, 1080]))
        );
        assert_eq!(
            snapshot_window("loading-thunderbirds-1920x1080"),
            Some((1, [1920, 1080]))
        );
        assert_eq!(snapshot_window("normal"), None);
    }
}

#[cfg(test)]
mod profile_probe {
    use super::*;
    /// Explicit local-only measurement. The path must be a private profile copy.
    /// `TORE_LOADING_THEATER` selects the theater (default `UKR`).
    /// `TORE_LOADING_REPEAT=1` times a second build in the same process.
    #[test]
    #[ignore = "requires TORE_LOADING_PROFILE pointing to a private imported profile"]
    fn real_mission_worker_services_headless_heartbeat() {
        let path = std::env::var_os("TORE_LOADING_PROFILE").expect("private profile path");
        let theater = std::env::var("TORE_LOADING_THEATER").unwrap_or_else(|_| "UKR".to_string());
        eprintln!("loading theater: {theater}");
        let assets = crate::assets::Assets::load(std::path::Path::new(&path)).unwrap();
        let resources = assets.theater_resources;
        let player =
            crate::aircraft::Airframe::load(&resources, tore_formats::aircraft::AircraftId::F18)
                .unwrap();
        let spec =
            crate::mission::MissionSpec::new(&theater, tore_formats::aircraft::AircraftId::F18);
        // A second build shows the process-wide city-fill layout cache. The
        // copy is outside the heartbeat timer.
        let resources_repeat = std::env::var_os("TORE_LOADING_REPEAT")
            .is_some()
            .then(|| resources.clone());
        let job =
            Job::spawn(move |progress| prepare_mission(&spec, &resources, player.kind, &progress))
                .unwrap();
        let started = Instant::now();
        let mut last = started;
        let mut max_gap = std::time::Duration::ZERO;
        let mut polls = 0;
        loop {
            let now = Instant::now();
            max_gap = max_gap.max(now - last);
            last = now;
            polls += 1;
            if let Some(result) = job.poll() {
                result.unwrap();
                break;
            }
            assert!(started.elapsed().as_secs() < 120, "loader did not finish");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        eprintln!(
            "loading probe: total_ms={:.3} heartbeat_max_ms={:.3} polls={polls}",
            started.elapsed().as_secs_f64() * 1000.,
            max_gap.as_secs_f64() * 1000.
        );
        assert!(polls > 1);
        assert!(max_gap < std::time::Duration::from_millis(250));
        if let Some(resources) = resources_repeat {
            let spec =
                crate::mission::MissionSpec::new(&theater, tore_formats::aircraft::AircraftId::F18);
            let player = crate::aircraft::Airframe::load(
                &resources,
                tore_formats::aircraft::AircraftId::F18,
            )
            .unwrap();
            let again = Instant::now();
            let job = Job::spawn(move |progress| {
                prepare_mission(&spec, &resources, player.kind, &progress)
            })
            .unwrap();
            loop {
                if let Some(result) = job.poll() {
                    result.unwrap();
                    break;
                }
                assert!(
                    again.elapsed().as_secs() < 120,
                    "cached loader did not finish"
                );
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            eprintln!(
                "loading cache repeat: total_ms={:.3}",
                again.elapsed().as_secs_f64() * 1000.
            );
        }
    }
}

/// The app lends its world to a restart worker only while the loading screen
/// owns event handling. No simulation step can run until it is returned.
pub(crate) struct WorldSlot(pub Option<crate::world::World>);
impl std::ops::Deref for WorldSlot {
    type Target = crate::world::World;
    fn deref(&self) -> &Self::Target {
        self.0.as_ref().expect("world is in the loader")
    }
}
impl std::ops::DerefMut for WorldSlot {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut().expect("world is in the loader")
    }
}

/// Reports remaining synchronous stalls without touching simulation clocks.
pub(crate) struct Timing(&'static str, Instant);
pub(crate) fn timing(name: &'static str) -> Timing {
    Timing(name, Instant::now())
}
impl Drop for Timing {
    fn drop(&mut self) {
        log::info!(
            "loading timing: {} {:.3} ms",
            self.0,
            self.1.elapsed().as_secs_f64() * 1000.
        );
    }
}

/// Command-line view options restored only after the creator's worker completes.
pub(crate) type CreatorView = (u8, crate::flight_views::Reference, [f32; 2], f32);

/// The existing creator smoke check, performed before the first flight frame.
/// It uses the same restart as a player and leaves the restarted world ready.
pub(crate) fn verify_restart(
    world: &mut crate::world::World,
    aircraft: &tore_world::aircraft_type::AircraftType,
    resources: &std::collections::BTreeMap<String, Vec<u8>>,
) -> Result<crate::world::Restarted, String> {
    let own = &world.cockpits[crate::OWN];
    let initial = (
        own.flight.position,
        own.flight.yaw,
        own.flight.speed,
        own.flight.gear,
        own.flight.fuel,
        own.flight.payload_lbs,
        own.airport_service.selected(),
    );
    let targets: Vec<_> = world
        .combat
        .state
        .targets
        .iter()
        .map(|t| (t.id, t.position, t.hp))
        .collect();
    let restart = world
        .restart(aircraft, resources)
        .map_err(|e| e.to_string())?;
    let own = &world.cockpits[crate::OWN];
    let restarted = (
        own.flight.position,
        own.flight.yaw,
        own.flight.speed,
        own.flight.gear,
        own.flight.fuel,
        own.flight.payload_lbs,
        own.airport_service.selected(),
    );
    let restarted_targets: Vec<_> = world
        .combat
        .state
        .targets
        .iter()
        .map(|t| (t.id, t.position, t.hp))
        .collect();
    if initial != restarted || targets != restarted_targets {
        return Err("Quick Mission restart did not restore the accepted start".into());
    }
    println!("Quick Mission restart: PASS");
    Ok(restart)
}
