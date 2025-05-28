use bevy::{platform::collections::HashMap, prelude::*};

use crate::{SelectedFace, common_assets::Common};

#[derive(Clone, Eq, PartialEq, Debug, Hash)]
pub struct VoxelInfo {
    pub material: Handle<StandardMaterial>,
    pub rendered: Option<Entity>,
}

#[allow(unused)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub enum SymmetryKind {
    None,
    Rotation,
    MirrorX,
}

/// A function to apply an undo.
type UndoFunction = dyn FnOnce(&mut Voxels, &mut Commands, &Common) + 'static + Send + Sync;

#[derive(Resource)]
pub struct Voxels {
    symmetry: SymmetryKind,
    voxel_fill: HashMap<IVec3, VoxelInfo>,
    /// A shift to apply to all voxels in a column.
    /// This shift should be less than half the voxel grid size.
    column_shift: HashMap<IVec2, i32>,

    /// Functions to undo operations to the voxel data.
    undo_log: Vec<Box<UndoFunction>>,

    /// The editor state just before applying the last action.
    pub editor_state_before: Option<CommittedEditorState>,

    /// The index to roll back to when performing an 'undo'.
    undo_commit_indexes: Vec<(usize, CommittedEditorState)>,
}

#[derive(Component)]
pub struct VoxelMarker(pub IVec3);

impl VoxelMarker {
    /// Return the center of the voxel in world space.
    pub fn center(&self) -> Vec3 {
        Vec3::splat(VOXEL_SIZE) * self.0.as_vec3()
    }
}

/// A snapshot of the editor state, for applying undos.
#[derive(Clone, Debug)]
pub struct CommittedEditorState {
    pub selection: Vec<SelectedFace>,
}

impl Voxels {
    /// Adds a function to the undo log.
    fn add_undo_log(
        &mut self,
        f: impl FnOnce(&mut Voxels, &mut Commands, &Common) + 'static + Send + Sync,
    ) {
        self.undo_log.push(Box::new(f));
    }

    pub fn new_empty() -> Self {
        Self {
            symmetry: SymmetryKind::Rotation,
            voxel_fill: HashMap::new(),
            column_shift: HashMap::new(),
            undo_log: Vec::new(),
            editor_state_before: None,
            undo_commit_indexes: Vec::new(),
        }
    }

    pub fn apply_symmetry(&self, voxel: IVec3) -> Option<IVec3> {
        if voxel.xz() == IVec2::ZERO {
            return None;
        }

        match self.symmetry {
            SymmetryKind::None => None,
            SymmetryKind::Rotation => Some(IVec3::new(-voxel.x, voxel.y, -voxel.z)),
            SymmetryKind::MirrorX => Some(IVec3::new(-voxel.x, voxel.y, voxel.z)),
        }
    }

    pub fn remove_voxel(&mut self, commands: &mut Commands, voxel: IVec3) {
        self.remove_voxel_internal(commands, voxel);
        if let Some(voxel) = self.apply_symmetry(voxel) {
            self.remove_voxel_internal(commands, voxel);
        }
    }

    fn remove_voxel_internal(&mut self, commands: &mut Commands, voxel: IVec3) {
        if voxel == IVec3::ZERO {
            return;
        }
        let Some(mut voxel_info) = self.voxel_fill.remove(&voxel) else {
            return;
        };
        let rendered_entity = voxel_info.rendered.take();

        self.add_undo_log(move |voxels, commands, common| {
            // Re-insert and re-render the removed voxel.
            voxels.voxel_fill.insert(voxel, voxel_info);
            voxels.redraw_voxel(commands, common, voxel);
        });

        if let Some(entity) = rendered_entity {
            commands.entity(entity).despawn();
        }
    }
    pub fn add_voxel(
        &mut self,
        commands: &mut Commands,
        common: &Common,
        voxel: IVec3,
        mat: Handle<StandardMaterial>,
    ) {
        self.add_voxel_internal(commands, common, voxel, mat.clone());

        if let Some(voxel) = self.apply_symmetry(voxel) {
            let complement_material = if mat == common.red_material {
                common.blue_material.clone()
            } else if mat == common.blue_material {
                common.red_material.clone()
            } else {
                mat
            };

            self.add_voxel_internal(commands, common, voxel, complement_material);
        }
    }
    pub fn add_voxel_internal(
        &mut self,
        commands: &mut Commands,
        common: &Common,
        voxel: IVec3,
        mat: Handle<StandardMaterial>,
    ) {
        // Remove the voxel already present at the location.
        self.remove_voxel_internal(commands, voxel);
        self.voxel_fill.insert(
            voxel,
            VoxelInfo {
                material: mat.clone(),
                rendered: None,
            },
        );
        self.redraw_voxel(commands, common, voxel);

        self.add_undo_log(move |voxels, commands, _common| {
            let Some(mut voxel_info) = voxels.voxel_fill.remove(&voxel) else {
                return;
            };
            if let Some(rendered_entity) = voxel_info.rendered.take() {
                commands.entity(rendered_entity).despawn();
            }
        });
    }
    pub fn has_voxel(&self, voxel: IVec3) -> bool {
        self.voxel_fill.contains_key(&voxel)
    }

    /// Gets the voxel at the specified location, if any.
    pub fn get_voxel(&self, voxel: IVec3) -> Option<&VoxelInfo> {
        self.voxel_fill.get(&voxel)
    }

    pub fn get_material(&self, voxel: IVec3) -> Option<Handle<StandardMaterial>> {
        Some(self.get_voxel(voxel)?.material.clone())
    }

    /// Despawns and re-spawns the voxel at the given location.
    pub fn redraw_voxel(&mut self, commands: &mut Commands, common: &Common, voxel: IVec3) {
        let column_shift = self.column_shift.get(&voxel.xz()).copied().unwrap_or(0);

        let Some(voxel_info) = self.voxel_fill.get_mut(&voxel) else {
            return;
        };

        if let Some(already_rendered) = voxel_info.rendered.take() {
            // Remove the previous version of the voxel.
            commands.entity(already_rendered).despawn();
        }

        let rendered = commands
            .spawn((
                VoxelMarker(voxel),
                Transform::from_translation(
                    Vec3::splat(VOXEL_SIZE) * voxel.as_vec3() + Vec3::Y * column_shift as f32,
                )
                .with_scale(Vec3::splat(VOXEL_SIZE)),
                Mesh3d(common.cube_mesh.clone()),
                MeshMaterial3d(voxel_info.material.clone()),
            ))
            .id();

        voxel_info.rendered = Some(rendered);
    }

    fn shift_column_internal(
        &mut self,
        commands: &mut Commands,
        common: &Common,
        column: IVec2,
        by: i32,
    ) {
        // TODO: Place a limit on this.
        *self.column_shift.entry(column).or_default() += by;
        self.add_undo_log(move |voxels, commands, common| {
            *voxels.column_shift.entry(column).or_default() -= by;

            for voxel in voxels
                .voxel_fill
                .keys()
                .copied()
                .filter(|v| v.xz() == column)
                .collect::<Vec<IVec3>>()
            {
                voxels.redraw_voxel(commands, common, voxel);
            }
        });

        for voxel in self
            .voxel_fill
            .keys()
            .copied()
            .filter(|v| v.xz() == column)
            .collect::<Vec<IVec3>>()
        {
            self.redraw_voxel(commands, common, voxel);
        }
    }

    /// Shifts the target column up or down.
    pub fn shift_column(
        &mut self,
        commands: &mut Commands,
        common: &Common,
        column: IVec2,
        by: i32,
    ) {
        self.shift_column_internal(commands, common, column, by);
        if let Some(symmetric_voxel) = self.apply_symmetry(IVec3::new(column.x, 0, column.y)) {
            self.shift_column_internal(commands, common, symmetric_voxel.xz(), by);
        }
    }

    /// Returns whether there are changes to commit.
    pub fn has_changes_to_commit(&self) -> bool {
        self.undo_commit_indexes
            .last()
            .map(|record| record.0)
            .map(|index| index != self.undo_log.len())
            .unwrap_or(true)
    }

    /// Save all of the most-recent changes in the undo log, so that they will be undone as a unit.
    ///
    /// Call `has_changes_to_commit` before calling this function.
    pub fn commit_changes(&mut self, editor_state: CommittedEditorState) {
        self.undo_commit_indexes
            .push((self.undo_log.len(), editor_state));
    }

    /// Applies the undo functions for the last action.
    pub fn undo_last_action(
        &mut self,
        commands: &mut Commands,
        common: &Common,
    ) -> CommittedEditorState {
        static EMPTY_EDITOR_STATE: CommittedEditorState = CommittedEditorState {
            selection: Vec::new(),
        };

        let last_editor_state = self
            .undo_commit_indexes
            .pop()
            .map(|(_, last_editor_state)| last_editor_state)
            .unwrap_or_else(|| EMPTY_EDITOR_STATE.clone());

        let undo_until = self
            .undo_commit_indexes
            .last()
            .map(|pair| pair.0)
            .unwrap_or(0);

        while self.undo_log.len() > undo_until {
            let undo_func = self.undo_log.pop().unwrap();
            undo_func(self, commands, common);
        }

        last_editor_state
    }

    /// Iterates through all of the voxels in the grid.
    pub fn iter_voxels(&self) -> impl Iterator<Item = (IVec3, &VoxelInfo)> {
        self.voxel_fill.iter().map(|(p, v)| (*p, v))
    }
}

pub const VOXEL_SIZE: f32 = 128.0;
