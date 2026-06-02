sets = [
    ('MemMapSparseSet', 'MemoryMap', 'memmap_set'),
    ('IpcCapSparseSet', 'IpcCapability', 'ipc_cap_set'),
    ('SocketSparseSet', 'OpenSocket', 'socket_set'),
    ('AffinitySparseSet', 'CpuAffinity', 'affinity_set'),
    ('EpochInfoSparseSet', 'EpochInfo', 'epoch_info_set'),
    ('PerfCountersSparseSet', 'PerfCounters', 'perf_counters_set'),
    ('TestSparseSet', 'TestComponent', 'test_set')
]

out = []

for struct_name, comp_type, prefix in sets:
    out.append(f"""
pub struct {struct_name} {{
    pub dense: Ptr<{comp_type}>,
    pub sparse: Ptr<u32>,
    pub entity_map: Ptr<Entity>,
    pub count: u32,
}}

pub fn new_{prefix}() -> Ptr<{struct_name}> {{
    let t_size = size_of::<{comp_type}>();
    let d_ptr = malloc((MAX_ENTITIES as i64) * t_size) as Ptr<{comp_type}>;
    let s_ptr = malloc((MAX_ENTITIES as i64) * 4) as Ptr<u32>;
    let e_ptr = malloc((MAX_ENTITIES as i64) * 8) as Ptr<Entity>;
    
    let mut i: u32 = 0;
    while i < MAX_ENTITIES {{
        (*(( (s_ptr as u64) + (i as u64) * 4 ) as &mut u32)) = 0xFFFFFFFF;
        i = i + 1;
    }}
    
    let p = malloc(size_of::<{struct_name}>() as i64) as Ptr<{struct_name}>;
    p[0] = {struct_name} {{ dense: d_ptr, sparse: s_ptr, entity_map: e_ptr, count: 0 }};
    return p;
}}

pub fn {prefix}_insert(set: Ptr<{struct_name}>, entity: Entity, component: {comp_type}) {{
    let id = entity.id;
    if id >= MAX_ENTITIES {{ return; }}
    let s_ptr = set[0].sparse;
    let dense_idx = (*(( (s_ptr as u64) + (id as u64) * 4 ) as &u32));
    if dense_idx != 0xFFFFFFFF {{
        (*(( (set[0].dense as u64) + (dense_idx as u64) * (size_of::<{comp_type}>() as u64) ) as &mut {comp_type})) = component;
        (*(( (set[0].entity_map as u64) + (dense_idx as u64) * 8 ) as &mut Entity)) = entity;
        return;
    }}
    let count = set[0].count;
    if count >= MAX_ENTITIES {{ return; }}
    (*(( (set[0].dense as u64) + (count as u64) * (size_of::<{comp_type}>() as u64) ) as &mut {comp_type})) = component;
    (*(( (set[0].entity_map as u64) + (count as u64) * 8 ) as &mut Entity)) = entity;
    (*(( (set[0].sparse as u64) + (id as u64) * 4 ) as &mut u32)) = count;
    set[0].count = count + 1;
}}

pub fn {prefix}_get(set: Ptr<{struct_name}>, entity: Entity) -> Ptr<{comp_type}> {{
    let sid = entity.id;
    if sid >= MAX_ENTITIES {{ return 0 as Ptr<{comp_type}>; }}
    let dense_idx = (*(( (set[0].sparse as u64) + (sid as u64) * 4 ) as &u32));
    if dense_idx != 0xFFFFFFFF {{
        if (*(( (set[0].entity_map as u64) + (dense_idx as u64) * 8 ) as &Entity)).gen == entity.gen {{
            return ((set[0].dense as u64) + (dense_idx as u64) * (size_of::<{comp_type}>() as u64)) as Ptr<{comp_type}>;
        }}
    }}
    return 0 as Ptr<{comp_type}>;
}}

pub fn {prefix}_remove(set: Ptr<{struct_name}>, entity: Entity) {{
    let sid = entity.id;
    if sid >= MAX_ENTITIES {{ return; }}
    let dense_idx = (*(( (set[0].sparse as u64) + (sid as u64) * 4 ) as &u32));
    if dense_idx != 0xFFFFFFFF {{
        if (*(( (set[0].entity_map as u64) + (dense_idx as u64) * 8 ) as &Entity)).gen == entity.gen {{
            let last_idx = set[0].count - 1;
            if dense_idx != last_idx {{
                (*(( (set[0].dense as u64) + (dense_idx as u64) * (size_of::<{comp_type}>() as u64) ) as &mut {comp_type})) = (*(( (set[0].dense as u64) + (last_idx as u64) * (size_of::<{comp_type}>() as u64) ) as &{comp_type}));
                let last_entity = (*(( (set[0].entity_map as u64) + (last_idx as u64) * 8 ) as &Entity));
                (*(( (set[0].entity_map as u64) + (dense_idx as u64) * 8 ) as &mut Entity)) = last_entity;
                (*(( (set[0].sparse as u64) + (last_entity.id as u64) * 4 ) as &mut u32)) = dense_idx;
            }}
            (*(( (set[0].sparse as u64) + (sid as u64) * 4 ) as &mut u32)) = 0xFFFFFFFF;
            set[0].count = last_idx;
        }}
    }}
}}
""")

with open('kernel/ecs/sparse_set.salt', 'w') as f:
    text = open('kernel/ecs/sparse_set.salt.bak', 'r').read()
    f.write(text + '\n'.join(out))
