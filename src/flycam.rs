use bevy::{input::mouse::MouseMotion, prelude::*};

pub struct FlyCameraPlugin;

impl Plugin for FlyCameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, control_camera_system);
    }
}

#[derive(Component)]
pub struct CameraControls {
    speed: f32,
}

impl Default for CameraControls {
    fn default() -> Self {
        Self { speed: 512. }
    }
}

fn control_camera_system(
    time: Res<Time>,
    mut camera: Query<(&mut Transform, &mut CameraControls)>,
    key: Res<ButtonInput<KeyCode>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut mouse_move: EventReader<MouseMotion>,
) {
    for (mut camera_transform, controls) in camera.iter_mut() {
        let forward = camera_transform.forward();
        let right = camera_transform.right();

        let mut local: Vec3 = Vec3::ZERO;
        if key.pressed(KeyCode::KeyD) {
            local.x += 1.0;
        }
        if key.pressed(KeyCode::KeyA) {
            local.x -= 1.0;
        }
        if key.pressed(KeyCode::KeyW) {
            local.z += 1.0;
        }
        if key.pressed(KeyCode::KeyS) {
            local.z -= 1.0;
        }

        camera_transform.translation +=
            (local.x * right + local.z * forward) * controls.speed * time.delta_secs();

        if mouse_button.pressed(MouseButton::Right) {
            let mut angle_azimuth = forward.z.atan2(forward.x);
            let mut angle_altitude = forward.y.asin();

            let rot_speed = 0.005;
            for evt in mouse_move.read() {
                angle_azimuth += evt.delta.x * rot_speed;
                angle_altitude -= evt.delta.y * rot_speed;
            }

            angle_altitude =
                angle_altitude.clamp(-std::f32::consts::PI * 0.99, std::f32::consts::PI * 0.99);

            camera_transform.look_to(
                Vec3::new(
                    angle_azimuth.cos() * angle_altitude.cos(),
                    angle_altitude.sin(),
                    angle_azimuth.sin() * angle_altitude.cos(),
                ),
                Vec3::Y,
            );
            // ...
        }
    }
}
