//! Bounded worker and stable GPU pages. All candidate selection and packing
//! runs on the worker; the render thread only copies changed pages.
use super::{CacheFrame, INST_STRIDE, Library, Patch, Placement, Terrain, pack};
use crate::vegetation::{Atlas, Inst};
use std::{
    collections::BTreeMap,
    sync::{Arc, mpsc},
};

pub const PAGE: usize = 256;
// Two maximum 8 nm rings have 4034 chunks. Each can waste one partial page.
// Two object budgets plus this slack hold the published and staged selections.
pub const SPARE_PAGES: usize = 4096;
pub const UPLOAD_BYTES: usize = 256 * 1024;
pub const UPLOAD_WRITES: usize = 16;
pub type Key = (i64, i64);
pub type Chunks = Vec<(Key, Arc<Vec<u8>>)>;

#[derive(Clone, Copy)]
pub struct Request {
    pub epoch: u64,
    pub eye: [f64; 2],
    pub full: f32,
    pub draw: f32,
    pub density: f32,
    pub capacity: usize,
    pub grass_radius: f32,
    pub settle: bool,
}

pub struct Ready {
    pub epoch: u64,
    pub chunks: Chunks,
    pub grass: Vec<Patch>,
    pub squeeze: f32,
    pub reach: f32,
    pub done: bool,
    pub cache: CacheFrame,
}

pub struct Worker {
    send: mpsc::SyncSender<Request>,
    receive: mpsc::Receiver<Ready>,
    pub busy: bool,
}
impl Worker {
    pub fn new(atlas: Arc<Atlas>, library: Arc<Library>, world: &Terrain) -> Self {
        // Only immutable placement inputs are retained. No live simulation state
        // is shared with or modified by the presentation worker.
        let world = Terrain {
            theater: tore_formats::theater::Theater {
                name: world.theater.name.clone(),
                map: world.theater.map.clone(),
                tiles: world.theater.tiles,
                cells_per_tile: world.theater.cells_per_tile,
                cols: world.theater.cols,
                rows: world.theater.rows,
                cells: world.theater.cells.clone(),
                coarse: world.theater.coarse.clone(),
            },
            layout: world.layout.clone(),
            condition: world.condition,
            environment: Default::default(),
            airport_scene: world.airport_scene.clone(),
            airfield_anchors: BTreeMap::new(),
            static_manifest: Vec::new(),
            catalog: Vec::new(),
            weather: world.weather.clone(),
            surface: Default::default(),
            city_fill: world.city_fill.clone(),
            redrawn: Vec::new(),
            redrawn_airports: world.redrawn_airports,
            redrawn_templates: world.redrawn_templates,
            merged_airports: world.merged_airports,
            neutral_airfield_ground: world.neutral_airfield_ground.clone(),
        };
        let (send, requests) = mpsc::sync_channel::<Request>(1);
        let (results, receive) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("vegetation".into())
            .spawn(move || {
                let mut placement = Placement::new(atlas);
                let mut epoch = u64::MAX;
                let mut selected_generation = u64::MAX;
                let mut chunks = Vec::new();
                let mut previous: BTreeMap<Key, (Vec<Inst>, Arc<Vec<u8>>)> = BTreeMap::new();
                while let Ok(request) = requests.recv() {
                    if request.epoch != epoch {
                        placement.reset_cache();
                        previous.clear();
                        selected_generation = u64::MAX;
                        epoch = request.epoch;
                    }
                    placement.set_density(request.density);
                    placement.set_budget(request.capacity);
                    let before = (placement.builds, placement.grass_builds, placement.build_ns);
                    let done = placement.update(
                        &world,
                        request.eye,
                        request.full,
                        request.draw,
                        if request.settle { None } else { Some(3) },
                    );
                    let (grass, grass_done) = placement.grass(
                        &world,
                        request.eye,
                        request.grass_radius,
                        if request.settle { None } else { Some(2) },
                    );
                    if selected_generation != placement.generation {
                        let selected = placement.object_chunks(request.eye);
                        let mut next = BTreeMap::new();
                        chunks.clear();
                        for (key, objects) in selected {
                            let bytes = match previous.remove(&key) {
                                Some((old, bytes)) if old == objects => bytes,
                                _ => {
                                    let mut bytes =
                                        Vec::with_capacity(objects.len() * INST_STRIDE as usize);
                                    for i in &objects {
                                        let mesh = library.mesh_for(i.species, i.variant);
                                        for value in
                                            pack(i, mesh, library.meshes[usize::from(mesh)].radius)
                                        {
                                            bytes.extend_from_slice(&value.to_le_bytes());
                                        }
                                    }
                                    Arc::new(bytes)
                                }
                            };
                            chunks.push((key, bytes.clone()));
                            next.insert(key, (objects, bytes));
                        }
                        previous = next;
                        selected_generation = placement.generation;
                    }
                    let ready = Ready {
                        epoch,
                        chunks: chunks.clone(),
                        grass,
                        squeeze: placement.squeezed(1.),
                        reach: placement.kept_reach(request.eye),
                        done: done && grass_done,
                        cache: CacheFrame {
                            object_chunks: placement.builds - before.0,
                            grass_chunks: placement.grass_builds - before.1,
                            build_ms: (placement.build_ns - before.2) as f64 / 1_000_000.,
                            pending: !done || !grass_done,
                            submit_ms: 0.,
                        },
                    };
                    if results.send(ready).is_err() {
                        break;
                    }
                }
            })
            .expect("vegetation worker");
        Self {
            send,
            receive,
            busy: false,
        }
    }
    pub fn request(&mut self, request: Request) {
        if !self.busy && self.send.try_send(request).is_ok() {
            self.busy = true;
        }
    }
    pub fn poll(&mut self, wait: bool) -> Option<Ready> {
        if !self.busy {
            return None;
        }
        let ready = if wait {
            self.receive.recv().ok()
        } else {
            self.receive.try_recv().ok()
        };
        if ready.is_some() {
            self.busy = false;
        }
        ready
    }
}

#[derive(Clone)]
struct Resident {
    bytes: Arc<Vec<u8>>,
    pages: Vec<usize>,
    uploaded: usize,
}

/// Fixed pages avoid fragmentation. A chunk retains its pages until its
/// selected candidates change or it leaves. The table hides incomplete chunks.
pub struct Slots {
    residents: BTreeMap<Key, Resident>,
    order: Vec<Key>,
    staged: Option<(BTreeMap<Key, Resident>, Vec<Key>)>,
    free: Vec<usize>,
}
impl Slots {
    pub fn new(capacity: usize) -> Self {
        Self {
            residents: BTreeMap::new(),
            order: Vec::new(),
            staged: None,
            free: (0..2 * capacity.div_ceil(PAGE) + SPARE_PAGES)
                .rev()
                .collect(),
        }
    }
    pub fn sync(&mut self, chunks: &Chunks) {
        assert!(self.staged.is_none());
        let mut next = BTreeMap::new();
        let mut order = Vec::new();
        for (key, bytes) in chunks {
            order.push(*key);
            let resident = if let Some(old) = self
                .residents
                .get(key)
                .filter(|r| Arc::ptr_eq(bytes, &r.bytes))
            {
                old.clone()
            } else {
                let count = bytes.len().div_ceil(PAGE * INST_STRIDE as usize);
                assert!(count <= self.free.len(), "vegetation page budget");
                let pages = (0..count).map(|_| self.free.pop().unwrap()).collect();
                Resident {
                    bytes: bytes.clone(),
                    pages,
                    uploaded: 0,
                }
            };
            next.insert(*key, resident);
        }
        self.staged = Some((next, order));
    }
    pub fn upload(&mut self, budget: usize, mut write: impl FnMut(u64, &[u8])) {
        let Some((next, order)) = &mut self.staged else {
            return;
        };
        let mut left = budget;
        let mut calls = if budget == usize::MAX {
            usize::MAX
        } else {
            UPLOAD_WRITES
        };
        for key in order.iter() {
            let resident = next.get_mut(key).unwrap();
            while resident.uploaded < resident.bytes.len() {
                let start = resident.uploaded;
                let end = (start + PAGE * INST_STRIDE as usize).min(resident.bytes.len());
                if end - start > left || calls == 0 {
                    return;
                }
                let page = resident.pages[start / (PAGE * INST_STRIDE as usize)];
                write(
                    (page * PAGE) as u64 * INST_STRIDE,
                    &resident.bytes[start..end],
                );
                left -= end - start;
                calls -= 1;
                resident.uploaded = end;
            }
        }
        // Publish the whole selection atomically. The previous view remains
        // drawable during uploads, including chunks whose selection changed.
        let (next, order) = self.staged.take().unwrap();
        for (key, resident) in &self.residents {
            if !next
                .get(key)
                .is_some_and(|r| Arc::ptr_eq(&r.bytes, &resident.bytes))
            {
                self.free.extend(resident.pages.iter().copied());
            }
        }
        self.residents = next;
        self.order = order;
    }
    pub fn pending(&self) -> bool {
        self.staged.is_some()
    }
    pub fn table(&self) -> (Vec<u32>, u32) {
        let mut table = Vec::new();
        let mut total = 0;
        for key in &self.order {
            let r = &self.residents[key];
            if r.uploaded != r.bytes.len() {
                continue;
            }
            let count = r.bytes.len() / INST_STRIDE as usize;
            total += count as u32;
            for (index, page) in r.pages.iter().enumerate() {
                table.extend([
                    (page * PAGE) as u32,
                    (count - index * PAGE).min(PAGE) as u32,
                ]);
            }
        }
        (table, total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn chunk(n: usize, value: u8) -> Arc<Vec<u8>> {
        Arc::new(vec![value; n * INST_STRIDE as usize])
    }
    #[test]
    fn insert_retire_reuse_without_repacking_existing_chunks() {
        let a = chunk(300, 1);
        let b = chunk(60, 2);
        let mut slots = Slots::new(1024);
        slots.sync(&vec![((0, 0), a.clone())]);
        let mut writes = Vec::new();
        slots.upload(usize::MAX, |offset, bytes| {
            writes.push((offset, bytes.to_vec()))
        });
        let old = slots.table();
        assert_eq!(old.1, 300);
        slots.sync(&vec![((0, 0), a.clone()), ((1, 0), b.clone())]);
        writes.clear();
        slots.upload(usize::MAX, |offset, bytes| {
            writes.push((offset, bytes.to_vec()))
        });
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0].1, *b);
        assert_eq!(&slots.table().0[..old.0.len()], &old.0);
        let released = slots.residents[&(0, 0)].pages.clone();
        slots.sync(&vec![((1, 0), b.clone())]);
        slots.upload(usize::MAX, |_, _| {});
        slots.sync(&vec![((1, 0), b), ((2, 0), chunk(300, 3))]);
        slots.upload(usize::MAX, |_, _| {});
        assert!(
            slots.residents[&(2, 0)]
                .pages
                .iter()
                .all(|p| released.contains(p))
        );
    }
    #[test]
    fn tiny_chunks_are_bounded_by_upload_call_count_as_well_as_bytes() {
        let mut slots = Slots::new(1024);
        let chunks: Chunks = (0..40).map(|x| ((x, 0), chunk(1, x as u8))).collect();
        slots.sync(&chunks);
        let mut calls = 0;
        slots.upload(UPLOAD_BYTES, |_, _| calls += 1);
        assert_eq!(calls, UPLOAD_WRITES);
        assert!(slots.pending());
        assert_eq!(slots.table().1, 0);
        slots.upload(usize::MAX, |_, _| {});
        assert!(!slots.pending());
        assert_eq!(slots.table().1, 40);
    }

    #[test]
    fn replacement_keeps_previous_selection_until_all_pages_arrive() {
        let mut slots = Slots::new(1024);
        slots.sync(&vec![((0, 0), chunk(300, 1))]);
        slots.upload(usize::MAX, |_, _| {});
        let previous = slots.table();
        slots.sync(&vec![((0, 0), chunk(500, 2))]);
        let mut uploaded = 0;
        slots.upload(PAGE * INST_STRIDE as usize, |_, bytes| {
            uploaded += bytes.len()
        });
        assert_eq!(uploaded, PAGE * INST_STRIDE as usize);
        assert_eq!(slots.table(), previous);
        assert!(slots.pending());
        slots.upload(PAGE * INST_STRIDE as usize, |_, _| {});
        assert!(!slots.pending());
        assert_eq!(slots.table().1, 500);
        slots.sync(&Vec::new());
        slots.upload(0, |_, _| panic!("retirement must not upload"));
        assert_eq!(slots.table().1, 0);
    }

    #[test]
    fn budget_hides_partial_chunks_and_table_recovers_exact_selected_set() {
        let mut slots = Slots::new(1024);
        let chunks = vec![((0, 0), chunk(300, 1)), ((1, 0), chunk(20, 2))];
        slots.sync(&chunks);
        let mut memory = vec![0; (2048 + SPARE_PAGES * PAGE) * INST_STRIDE as usize];
        slots.upload(PAGE * INST_STRIDE as usize, |offset, bytes| {
            memory[offset as usize..offset as usize + bytes.len()].copy_from_slice(bytes);
        });
        assert!(slots.pending());
        assert_eq!(slots.table().1, 0);
        slots.upload(usize::MAX, |offset, bytes| {
            memory[offset as usize..offset as usize + bytes.len()].copy_from_slice(bytes);
        });
        let (table, count) = slots.table();
        assert_eq!(count, 320);
        let mut gathered = Vec::new();
        for page in table.chunks_exact(2) {
            let start = page[0] as usize * INST_STRIDE as usize;
            gathered
                .extend_from_slice(&memory[start..start + page[1] as usize * INST_STRIDE as usize]);
        }
        let expected: Vec<u8> = chunks.iter().flat_map(|(_, b)| b.iter().copied()).collect();
        assert_eq!(gathered, expected);
    }
}
