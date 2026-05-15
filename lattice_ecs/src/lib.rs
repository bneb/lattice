#![no_std]

extern crate alloc;

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::any::{Any, TypeId};
use hashbrown::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Entity(pub u32);

pub trait Component: Any + 'static {}
impl<T: Any + 'static> Component for T {}

pub trait Resource: Any + 'static {}
impl<T: Any + 'static> Resource for T {}

pub trait ComponentStorage {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub struct SparseSet<T> {
    dense: Vec<T>,
    sparse: HashMap<Entity, usize>,
    pub dense_to_entity: Vec<Entity>,
}

impl<T> SparseSet<T> {
    fn new() -> Self {
        Self {
            dense: Vec::new(),
            sparse: HashMap::new(),
            dense_to_entity: Vec::new(),
        }
    }

    fn insert(&mut self, entity: Entity, component: T) {
        if let Some(&idx) = self.sparse.get(&entity) {
            self.dense[idx] = component;
        } else {
            let idx = self.dense.len();
            self.dense.push(component);
            self.dense_to_entity.push(entity);
            self.sparse.insert(entity, idx);
        }
    }

    pub fn get(&self, entity: Entity) -> Option<&T> {
        self.sparse.get(&entity).map(|&idx| &self.dense[idx])
    }

    pub fn get_mut(&mut self, entity: Entity) -> Option<&mut T> {
        self.sparse.get(&entity).map(|&idx| &mut self.dense[idx])
    }
}

impl<T: 'static> ComponentStorage for SparseSet<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

pub struct World {
    next_entity: u32,
    components: HashMap<TypeId, Box<dyn ComponentStorage>>,
    resources: HashMap<TypeId, Box<dyn Any>>,
}

impl World {
    pub fn new() -> Self {
        Self {
            next_entity: 0,
            components: HashMap::new(),
            resources: HashMap::new(),
        }
    }

    pub fn spawn<B: Bundle>(&mut self, components: B) -> Entity {
        let entity = Entity(self.next_entity);
        self.next_entity += 1;
        components.insert(self, entity);
        entity
    }

    pub fn insert<T: Component>(&mut self, entity: Entity, component: T) {
        let type_id = TypeId::of::<T>();
        let storage = self
            .components
            .entry(type_id)
            .or_insert_with(|| Box::new(SparseSet::<T>::new()));
        let sparse_set = storage.as_any_mut().downcast_mut::<SparseSet<T>>().unwrap();
        sparse_set.insert(entity, component);
    }

    pub fn insert_resource<R: Resource>(&mut self, resource: R) {
        self.resources.insert(TypeId::of::<R>(), Box::new(resource));
    }

    pub fn get_resource<R: Resource>(&self) -> Option<&R> {
        self.resources
            .get(&TypeId::of::<R>())
            .and_then(|r| r.downcast_ref::<R>())
    }

    pub fn get_resource_mut<R: Resource>(&mut self) -> Option<&mut R> {
        self.resources
            .get_mut(&TypeId::of::<R>())
            .and_then(|r| r.downcast_mut::<R>())
    }

    pub fn get_storage<T: Component>(&self) -> Option<&SparseSet<T>> {
        self.components
            .get(&TypeId::of::<T>())
            .and_then(|s| s.as_any().downcast_ref::<SparseSet<T>>())
    }

    pub fn get_storage_mut<T: Component>(&mut self) -> Option<&mut SparseSet<T>> {
        self.components
            .get_mut(&TypeId::of::<T>())
            .and_then(|s| s.as_any_mut().downcast_mut::<SparseSet<T>>())
    }

    pub fn get<T: Component>(&self, entity: Entity) -> Option<&T> {
        self.get_storage::<T>()?.get(entity)
    }

    pub fn get_mut<T: Component>(&mut self, entity: Entity) -> Option<&mut T> {
        self.get_storage_mut::<T>()?.get_mut(entity)
    }

    // A hacky query implementation for the POC that extracts pointers to bypass borrow checker
    pub fn query<Q: QueryParams>(&mut self) -> Q::Iter<'_> {
        Q::query(self)
    }
}

pub trait Bundle {
    fn insert(self, world: &mut World, entity: Entity);
}

impl<T1: Component, T2: Component> Bundle for (T1, T2) {
    fn insert(self, world: &mut World, entity: Entity) {
        world.insert(entity, self.0);
        world.insert(entity, self.1);
    }
}

pub trait QueryParams {
    type Iter<'a>;
    fn query(world: &mut World) -> Self::Iter<'_>;
}

pub trait Command {
    fn apply(self: Box<Self>, world: &mut World);
}

pub trait System {
    fn run(&mut self, world: &mut World, commands: &mut Commands);
}

pub struct Schedule {
    systems: Vec<Box<dyn System>>,
}

pub struct Events<T> {
    events: Vec<T>,
}

impl<T> Events<T> {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub fn send(&mut self, event: T) {
        self.events.push(event);
    }
}

pub struct EventReader<T> {
    last_event_count: usize,
    _marker: core::marker::PhantomData<T>,
}

impl<T> EventReader<T> {
    pub fn new() -> Self {
        Self {
            last_event_count: 0,
            _marker: core::marker::PhantomData,
        }
    }

    pub fn iter<'a>(&mut self, events: &'a Events<T>) -> core::slice::Iter<'a, T> {
        let start = self.last_event_count;
        self.last_event_count = events.events.len();
        events.events[start..].iter()
    }
}

impl Schedule {
    pub fn new() -> Self {
        Self { systems: Vec::new() }
    }

    pub fn add_system<S: System + 'static>(&mut self, system: S) {
        self.systems.push(Box::new(system));
    }

    pub fn run(&mut self, world: &mut World) {
        let mut commands = Commands::new();
        for system in &mut self.systems {
            system.run(world, &mut commands);
        }
        commands.apply(world);
    }
}

pub struct Commands {
    queue: Vec<Box<dyn Command>>,
}

impl Commands {
    pub fn new() -> Self {
        Self { queue: Vec::new() }
    }

    pub fn push<C: Command + 'static>(&mut self, command: C) {
        self.queue.push(Box::new(command));
    }

    pub fn spawn<B: Bundle + 'static>(&mut self, bundle: B) {
        self.push(SpawnCommand { bundle });
    }

    pub fn apply(&mut self, world: &mut World) {
        for cmd in self.queue.drain(..) {
            cmd.apply(world);
        }
    }
}

struct SpawnCommand<B: Bundle> {
    bundle: B,
}

impl<B: Bundle + 'static> Command for SpawnCommand<B> {
    fn apply(self: Box<Self>, world: &mut World) {
        world.spawn(self.bundle);
    }
}

// Implement QueryParams for (&T1, &mut T2)
impl<'a, T1: Component, T2: Component> QueryParams for (&'a T1, &'a mut T2) {
    type Iter<'b> = alloc::vec::IntoIter<(Entity, (&'b T1, &'b mut T2))>;

    fn query(world: &mut World) -> Self::Iter<'_> {
        let mut results = Vec::new();

        // Use unsafe pointer extraction to allow simultaneous mutable borrow
        let s1_ptr = world.get_storage::<T1>().map(|s| s as *const SparseSet<T1>);
        let s2_ptr = world.get_storage_mut::<T2>().map(|s| s as *mut SparseSet<T2>);

        if let (Some(s1), Some(s2)) = (s1_ptr, s2_ptr) {
            let s1 = unsafe { &*s1 };
            let s2 = unsafe { &mut *s2 };

            for &entity in &s1.dense_to_entity {
                if let (Some(c1), Some(c2)) = (s1.get(entity), s2.get_mut(entity)) {
                    // Safe because components of different types don't alias
                    let c1: &T1 = unsafe { &*(c1 as *const T1) };
                    let c2: &mut T2 = unsafe { &mut *(c2 as *mut T2) };
                    results.push((entity, (c1, c2)));
                }
            }
        }
        results.into_iter()
    }
}

// Implement QueryParams for (&T1, &T2)
impl<'a, T1: Component, T2: Component> QueryParams for (&'a T1, &'a T2) {
    type Iter<'b> = alloc::vec::IntoIter<(Entity, (&'b T1, &'b T2))>;

    fn query(world: &mut World) -> Self::Iter<'_> {
        let mut results = Vec::new();

        let s1_ptr = world.get_storage::<T1>().map(|s| s as *const SparseSet<T1>);
        let s2_ptr = world.get_storage::<T2>().map(|s| s as *const SparseSet<T2>);

        if let (Some(s1), Some(s2)) = (s1_ptr, s2_ptr) {
            let s1 = unsafe { &*s1 };
            let s2 = unsafe { &*s2 };

            for &entity in &s1.dense_to_entity {
                if let (Some(c1), Some(c2)) = (s1.get(entity), s2.get(entity)) {
                    let c1: &T1 = unsafe { &*(c1 as *const T1) };
                    let c2: &T2 = unsafe { &*(c2 as *const T2) };
                    results.push((entity, (c1, c2)));
                }
            }
        }
        results.into_iter()
    }
}

// Implement QueryParams for &mut T
impl<'a, T: Component> QueryParams for &'a mut T {
    type Iter<'b> = alloc::vec::IntoIter<(Entity, &'b mut T)>;

    fn query(world: &mut World) -> Self::Iter<'_> {
        let mut results = Vec::new();
        let s_ptr = world.get_storage_mut::<T>().map(|s| s as *mut SparseSet<T>);
        if let Some(s) = s_ptr {
            let s = unsafe { &mut *s };
            for i in 0..s.dense_to_entity.len() {
                let entity = s.dense_to_entity[i];
                if let Some(c) = s.get_mut(entity) {
                    let c: &mut T = unsafe { &mut *(c as *mut T) };
                    results.push((entity, c));
                }
            }
        }
        results.into_iter()
    }
}

// Implement QueryParams for &T
impl<'a, T: Component> QueryParams for &'a T {
    type Iter<'b> = alloc::vec::IntoIter<(Entity, &'b T)>;

    fn query(world: &mut World) -> Self::Iter<'_> {
        let mut results = Vec::new();
        let s_ptr = world.get_storage::<T>().map(|s| s as *const SparseSet<T>);
        if let Some(s) = s_ptr {
            let s = unsafe { &*s };
            for &entity in &s.dense_to_entity {
                if let Some(c) = s.get(entity) {
                    let c: &T = unsafe { &*(c as *const T) };
                    results.push((entity, c));
                }
            }
        }
        results.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MemoryMap {
        size: usize,
    }

    struct SchedulingPriority {
        level: u8,
    }

    struct HardwareClock {
        ticks: u64,
    }

    struct ProcessSpawnerSystem;

    impl System for ProcessSpawnerSystem {
        fn run(&mut self, world: &mut World, commands: &mut Commands) {
            if let Some(clock) = world.get_resource_mut::<HardwareClock>() {
                clock.ticks += 1;
                // Spawn a new process every tick
                commands.spawn((
                    MemoryMap { size: 4096 },
                    SchedulingPriority { level: 2 },
                ));
            }
        }
    }

    #[test]
    fn test_schedule_execution() {
        let mut world = World::new();
        world.insert_resource(HardwareClock { ticks: 0 });

        let mut schedule = Schedule::new();
        schedule.add_system(ProcessSpawnerSystem);

        // Run the schedule twice
        schedule.run(&mut world);
        schedule.run(&mut world);

        // Verify resource was updated
        let clock = world.get_resource::<HardwareClock>().unwrap();
        assert_eq!(clock.ticks, 2);

        // Verify two entities were spawned
        let mut count = 0;
        for _ in world.query::<(&MemoryMap, &SchedulingPriority)>() {
            count += 1;
        }
        assert_eq!(count, 2);
    }

    #[test]
    fn test_commands_queue() {
        let mut world = World::new();
        let mut commands = Commands::new();

        // Queue a spawn command
        commands.spawn((
            MemoryMap { size: 512 },
            SchedulingPriority { level: 1 },
        ));

        // State shouldn't be in the world yet
        assert!(world.get_storage::<MemoryMap>().is_none() || world.get_storage::<MemoryMap>().unwrap().dense.is_empty());

        // Apply commands
        commands.apply(&mut world);

        // Now it should be there
        let mut count = 0;
        for _ in world.query::<(&MemoryMap, &SchedulingPriority)>() {
            count += 1;
        }
        assert_eq!(count, 1);
    }

    #[test]
    fn test_global_resources() {
        let mut world = World::new();

        // Insert a global resource
        world.insert_resource(HardwareClock { ticks: 0 });

        // Retrieve and mutate it
        if let Some(mut clock) = world.get_resource_mut::<HardwareClock>() {
            clock.ticks += 100;
        }

        // Retrieve immutably and verify
        let clock = world.get_resource::<HardwareClock>().expect("Should have hardware clock");
        assert_eq!(clock.ticks, 100);
    }

    #[derive(Clone, Copy)]
    struct HardwareInterrupt {
        irq_number: u8,
    }

    #[test]
    fn test_event_subsystem() {
        let mut world = World::new();

        // Initialize the events resource
        world.insert_resource(Events::<HardwareInterrupt>::new());

        // Send an event
        if let Some(mut events) = world.get_resource_mut::<Events<HardwareInterrupt>>() {
            events.send(HardwareInterrupt { irq_number: 14 });
            events.send(HardwareInterrupt { irq_number: 15 });
        }

        // Read the events
        let mut reader = EventReader::<HardwareInterrupt>::new();
        let events = world.get_resource::<Events<HardwareInterrupt>>().expect("Events resource missing");
        
        let mut read_count = 0;
        for event in reader.iter(events) {
            assert!(event.irq_number == 14 || event.irq_number == 15);
            read_count += 1;
        }
        assert_eq!(read_count, 2);

        // Second read should yield 0 events
        let mut read_count2 = 0;
        for _ in reader.iter(events) {
            read_count2 += 1;
        }
        assert_eq!(read_count2, 0);
    }

    #[test]
    fn test_single_queries_and_lookups() {
        let mut world = World::new();

        let pid1 = world.spawn((MemoryMap { size: 1024 }, SchedulingPriority { level: 1 }));
        let pid2 = world.spawn((MemoryMap { size: 2048 }, SchedulingPriority { level: 2 }));

        // Direct lookup
        let mem1 = world.get::<MemoryMap>(pid1).expect("pid1 missing mem");
        assert_eq!(mem1.size, 1024);

        if let Some(mem2) = world.get_mut::<MemoryMap>(pid2) {
            mem2.size = 4096;
        }

        // Single component query
        let mut sum = 0;
        for (_entity, mem) in world.query::<&MemoryMap>() {
            sum += mem.size;
        }
        assert_eq!(sum, 1024 + 4096);

        // Mutable single query
        for (_entity, mem) in world.query::<&mut MemoryMap>() {
            mem.size += 1;
        }
        
        assert_eq!(world.get::<MemoryMap>(pid1).unwrap().size, 1025);
    }

    #[test]
    fn test_spawn_and_query_process() {
        let mut world = World::new();

        // Spawn a process entity with two components
        let pid = world.spawn((
            MemoryMap { size: 1024 * 1024 * 10 },
            SchedulingPriority { level: 5 },
        ));

        // Query for all entities with a MemoryMap and SchedulingPriority
        let mut found = false;
        for (entity, (mem, prio)) in world.query::<(&MemoryMap, &mut SchedulingPriority)>() {
            assert_eq!(entity, pid);
            assert_eq!(mem.size, 1024 * 1024 * 10);
            assert_eq!(prio.level, 5);
            
            // Mutate the priority
            prio.level = 10;
            found = true;
        }

        assert!(found, "Should have found the spawned process");

        // Verify the mutation persisted
        for (_entity, (_, prio)) in world.query::<(&MemoryMap, &SchedulingPriority)>() {
            assert_eq!(prio.level, 10);
        }
    }
}
