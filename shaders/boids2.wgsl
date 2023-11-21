struct type_4 {
    member: vec2<f32>,
    member_1: vec2<f32>,
}

struct type_5 {
    member: f32,
    member_1: f32,
    member_2: f32,
    member_3: f32,
    member_4: f32,
    member_5: f32,
    member_6: f32,
}

struct type_7 {
    member: array<type_4>,
}

struct type_9 {
    member: type_5,
}

@group(0) @binding(0) 
var<uniform> global: type_9;
@group(0) @binding(1) 
var<storage> global_1: type_7;
@group(0) @binding(2) 
var<storage, read_write> global_2: type_7;
var<private> global_3: vec3<u32>;

fn function() {
    var local: u32 = 0u;
    var local_1: i32 = 0;
    var local_2: vec2<f32> = vec2<f32>(0.0, 0.0);
    var local_3: vec2<f32> = vec2<f32>(0.0, 0.0);
    var local_4: vec2<f32> = vec2<f32>(0.0, 0.0);
    var local_5: vec2<f32> = vec2<f32>(0.0, 0.0);
    var local_6: vec2<f32> = vec2<f32>(0.0, 0.0);
    var local_7: vec2<f32> = vec2<f32>(0.0, 0.0);
    var local_8: i32 = 0;
    var local_9: vec2<f32> = vec2<f32>(0.0, 0.0);

    let _e42 = global_3;
    if (_e42.x >= 1500u) {
        return;
    }
    let _e49 = global_1.member[_e42.x].member;
    local_3 = _e49;
    let _e53 = global_1.member[_e42.x].member_1;
    local_6 = _e53;
    local_9 = vec2<f32>(0.0);
    local_2 = vec2<f32>(0.0);
    local_5 = vec2<f32>(0.0);
    local_8 = 0;
    local_1 = 0;
    local = 0u;
    loop {
        let _e57 = local;
        if (_e57 >= 1500u) {
            break;
        }
        let _e59 = local;
        if (_e59 == _e42.x) {
            continue;
        }
        let _e61 = local;
        let _e65 = global_1.member[_e61].member;
        local_4 = _e65;
        let _e66 = local;
        let _e70 = global_1.member[_e66].member_1;
        local_7 = _e70;
        let _e71 = local_4;
        let _e72 = local_3;
        let _e75 = global.member.member_1;
        if (distance(_e71, _e72) < _e75) {
            let _e77 = local_9;
            let _e78 = local_4;
            local_9 = (_e77 + _e78);
            let _e80 = local_8;
            local_8 = (_e80 + 1);
        }
        let _e82 = local_4;
        let _e83 = local_3;
        let _e86 = global.member.member_2;
        if (distance(_e82, _e83) < _e86) {
            let _e88 = local_5;
            let _e89 = local_4;
            let _e90 = local_3;
            local_5 = (_e88 - (_e89 - _e90));
        }
        let _e93 = local_4;
        let _e94 = local_3;
        let _e97 = global.member.member_3;
        if (distance(_e93, _e94) < _e97) {
            let _e99 = local_2;
            let _e100 = local_7;
            local_2 = (_e99 + _e100);
            let _e102 = local_1;
            local_1 = (_e102 + 1);
        }
        continue;
        continuing {
            let _e104 = local;
            local = (_e104 + 1u);
        }
    }
    let _e106 = local_8;
    if (_e106 > 0) {
        let _e108 = local_9;
        let _e109 = local_8;
        let _e113 = local_3;
        local_9 = ((_e108 / vec2<f32>(f32(_e109))) - _e113);
    }
    let _e115 = local_1;
    if (_e115 > 0) {
        let _e117 = local_2;
        let _e118 = local_1;
        local_2 = (_e117 / vec2<f32>(f32(_e118)));
    }
    let _e122 = local_6;
    let _e123 = local_9;
    let _e125 = global.member.member_4;
    let _e128 = local_5;
    let _e130 = global.member.member_5;
    let _e133 = local_2;
    let _e135 = global.member.member_6;
    local_6 = (((_e122 + (_e123 * _e125)) + (_e128 * _e130)) + (_e133 * _e135));
    let _e138 = local_6;
    let _e140 = local_6;
    local_6 = (normalize(_e138) * clamp(length(_e140), 0.0, 0.10000000149011612));
    let _e144 = local_3;
    let _e145 = local_6;
    let _e147 = global.member.member;
    local_3 = (_e144 + (_e145 * _e147));
    let _e151 = local_3[0u];
    if (_e151 < -1.0) {
        local_3[0u] = 1.0;
    }
    let _e155 = local_3[0u];
    if (_e155 > 1.0) {
        local_3[0u] = -1.0;
    }
    let _e159 = local_3[1u];
    if (_e159 < -1.0) {
        local_3[1u] = 1.0;
    }
    let _e163 = local_3[1u];
    if (_e163 > 1.0) {
        local_3[1u] = -1.0;
    }
    let _e166 = local_3;
    global_2.member[_e42.x].member = _e166;
    let _e170 = local_6;
    global_2.member[_e42.x].member_1 = _e170;
    return;
}

@compute @workgroup_size(64, 1, 1) 
fn main(@builtin(global_invocation_id) param: vec3<u32>) {
    global_3 = param;
    function();
}
