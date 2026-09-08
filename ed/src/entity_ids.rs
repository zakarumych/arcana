use std::{collections::BTreeMap, num::NonZeroU64, sync::Arc};

use arcana::ecs::{
    entity::{IdRange, IdRangeAllocator},
    world::World,
};
use parking_lot::Mutex;

const ID_END: u64 = 1 << 63;
const CHUNK_SIZE: u64 = 1 << 32;
const MIN_RESERVE: u64 = 1 << 24;

/// Shared source of fresh IDs for editor scenes and simulations.
#[derive(Clone)]
pub(crate) struct EntityIds {
    pool: Arc<Mutex<Pool>>,
}

struct Pool {
    next: u64,
    recycled: BTreeMap<u64, u64>,
}

impl EntityIds {
    pub fn new() -> Self {
        EntityIds {
            pool: Arc::new(Mutex::new(Pool {
                next: 1,
                recycled: BTreeMap::new(),
            })),
        }
    }

    pub fn new_world(&self) -> World {
        World::builder()
            .with_id_range_allocator(Box::new(self.clone()))
            .build()
    }
}

impl Pool {
    fn allocate(&mut self) -> IdRange {
        let recycled = self
            .recycled
            .iter()
            .next()
            .map(|(&start, &end)| (start, end));

        let (start, end) = match recycled {
            Some((start, end)) => {
                self.recycled.remove(&start);
                let chunk_end = end.min(start + CHUNK_SIZE);
                if chunk_end < end {
                    self.recycled.insert(chunk_end, end);
                }
                (start, chunk_end)
            }
            None => {
                let start = self.next;
                let end = ID_END.min(start + CHUNK_SIZE);
                assert!(end - start >= MIN_RESERVE, "Editor entity IDs exhausted");
                self.next = end;
                (start, end)
            }
        };

        IdRange {
            start: NonZeroU64::new(start).expect("Entity ID ranges start above zero"),
            end: NonZeroU64::new(end).expect("Entity ID ranges end above zero"),
        }
    }

    fn recycle(&mut self, range: IdRange) {
        if range.count() < MIN_RESERVE {
            return;
        }

        let mut start = range.start.get();
        let mut end = range.end.get();

        if let Some((&previous_start, &previous_end)) = self.recycled.range(..start).next_back() {
            debug_assert!(previous_end <= start);
            if previous_end == start {
                start = previous_start;
                self.recycled.remove(&previous_start);
            }
        }
        if let Some((&next_start, &next_end)) = self.recycled.range(start..).next() {
            debug_assert!(end <= next_start);
            if end == next_start {
                end = next_end;
                self.recycled.remove(&next_start);
            }
        }

        self.recycled.insert(start, end);
    }
}

// Safety: the shared mutex serializes leases from disjoint fresh or returned
// ranges. Only never-issued IDs returned by a lease holder enter the free pool.
unsafe impl IdRangeAllocator for EntityIds {
    fn allocate_range(&mut self) -> IdRange {
        self.pool.lock().allocate()
    }

    unsafe fn deallocate_range(&mut self, range: IdRange) {
        self.pool.lock().recycle(range);
    }
}

#[cfg(test)]
mod tests {
    use super::{CHUNK_SIZE, EntityIds, ID_END, IdRange, IdRangeAllocator, MIN_RESERVE};

    #[test]
    fn live_worlds_allocate_disjoint_ids() {
        let ids = EntityIds::new();
        let mut scene = ids.new_world();
        let mut simulation = ids.clone().new_world();

        let scene_spawned = scene.spawn_empty().id();
        let scene_reserved = scene.allocate().id();
        let simulation_spawned = simulation.spawn_empty().id();
        let simulation_reserved = simulation.allocate().id();

        assert_eq!(scene_spawned.bits(), 1);
        assert_eq!(scene_reserved.bits(), 2);
        assert_eq!(simulation_spawned.bits(), 2 * CHUNK_SIZE + 1);
        assert_eq!(simulation_reserved.bits(), 2 * CHUNK_SIZE + 2);
        assert!(simulation_reserved.bits() < ID_END);
    }

    #[test]
    fn dropped_world_returns_unused_suffix_without_issued_ids() {
        let ids = EntityIds::new();
        let mut world = ids.new_world();
        let spawned = world.spawn_empty().id();
        assert!(world.despawn(spawned).is_ok());
        let reserved = world.allocate().id();
        drop(world);

        let mut replacement = ids.new_world();
        assert_eq!(replacement.spawn_empty().id().bits(), reserved.bits() + 1);
        assert!(!replacement.is_alive(spawned));
        assert!(!replacement.is_alive(reserved));
    }

    #[test]
    fn copied_scene_id_coexists_with_fresh_simulation_entities() {
        let ids = EntityIds::new();
        let mut scene = ids.new_world();
        let copied = scene.spawn_empty().id();
        let mut simulation = ids.new_world();
        simulation.spawn_at(copied, ());
        let fresh = simulation.spawn_empty().id();
        drop(scene);

        let mut replacement = ids.new_world();
        let replacement_id = replacement.spawn_empty().id();
        assert!(simulation.is_alive(copied));
        assert!(simulation.is_alive(fresh));
        assert_ne!(fresh, copied);
        assert_ne!(replacement_id, copied);
        assert_ne!(replacement_id, fresh);
    }

    #[test]
    fn small_returned_fragments_are_discarded() {
        let mut ids = EntityIds::new();
        let original = ids.allocate_range();
        let tail = IdRange {
            start: original
                .get(CHUNK_SIZE - MIN_RESERVE + 1)
                .expect("Index is in range"),
            end: original.end,
        };
        // Safety: both returned subranges are unused, disjoint parts of original.
        unsafe { ids.deallocate_range(tail) };
        let fresh = ids.allocate_range();
        assert_eq!(fresh.start, original.end);
        unsafe {
            ids.deallocate_range(IdRange {
                start: original.start,
                end: tail.start,
            });
        }
        assert_eq!(
            ids.allocate_range(),
            IdRange {
                start: original.start,
                end: tail.start,
            }
        );
    }
}
