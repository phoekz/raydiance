use super::*;

pub struct MaterialEditorState {
    selected_material: usize,
}

impl MaterialEditorState {
    pub fn new() -> Self {
        Self {
            selected_material: 0,
        }
    }
}

pub struct MaterialEditor<'a> {
    state: &'a mut MaterialEditorState,
    rds_scene: &'a mut rds::Scene,
    dyn_scene: &'a mut rds::DynamicScene,
}

impl<'a> MaterialEditor<'a> {
    pub fn new(
        state: &'a mut MaterialEditorState,
        rds_scene: &'a mut rds::Scene,
        dyn_scene: &'a mut rds::DynamicScene,
    ) -> Self {
        Self {
            state,
            rds_scene,
            dyn_scene,
        }
    }
}

impl GuiElement for MaterialEditor<'_> {
    fn gui(&mut self, ui: &imgui::Ui) {
        // Selector.
        ui.combo(
            "Material",
            &mut self.state.selected_material,
            &self.rds_scene.materials,
            |material| Cow::Borrowed(&material.name),
        );

        // Model.
        {
            let material = &mut self.dyn_scene.materials[self.state.selected_material];
            let model = &mut material.model;
            if let Some(_token) = ui.begin_combo("Model", model.name()) {
                if ui.selectable(rds::MaterialModel::Diffuse.name()) {
                    *model = rds::MaterialModel::Diffuse;
                }
                if ui.selectable(rds::MaterialModel::Disney.name()) {
                    *model = rds::MaterialModel::Disney;
                }
            }
        }

        // Texture editor.
        if let Some(_token) = ui.begin_table("", 3) {
            let material = &self.dyn_scene.materials[self.state.selected_material];
            let base_color = material.base_color;
            let roughness = material.roughness;
            let metallic = material.metallic;
            let specular = material.specular;
            let specular_tint = material.specular_tint;
            let sheen = material.sheen;
            let sheen_tint = material.sheen_tint;
            ui.table_next_row();
            ui.table_set_column_index(0);
            base_color_gui(ui, "Base color", self.dyn_scene, base_color);
            scalar_gui(ui, "Roughness", self.dyn_scene, roughness);
            scalar_gui(ui, "Metallic", self.dyn_scene, metallic);
            scalar_gui(ui, "Specular", self.dyn_scene, specular);
            scalar_gui(ui, "Specular tint", self.dyn_scene, specular_tint);
            scalar_gui(ui, "Sheen", self.dyn_scene, sheen);
            scalar_gui(ui, "Sheen tint", self.dyn_scene, sheen_tint);
        }
    }
}

fn base_color_gui(
    ui: &imgui::Ui,
    name: &str,
    dyn_scene: &mut rds::DynamicScene,
    texture_index: u32,
) {
    let _id = ui.push_id(name);
    let index = texture_index as usize;
    let mut texture = &mut dyn_scene.textures[index];
    let mut bit = dyn_scene.replaced_textures[index];

    ui.text(name);
    ui.table_next_column();

    if let rds::DynamicTexture::Vector4(ref mut value) = &mut texture {
        if ui
            .color_edit4_config("Value", value)
            .alpha(false)
            .inputs(false)
            .build()
        {
            // Convenience: replace texture when an edit has been made without extra interaction.
            dyn_scene.replaced_textures.set(index, true);
        }
    }
    ui.table_next_column();

    {
        if ui.checkbox("##use", &mut bit) {
            dyn_scene.replaced_textures.set(index, bit);
        }
        ui.same_line();
        if ui.button("X") {
            // Convenience: reset to default value and clear replacement with one click.
            *texture = dyn_scene.default_textures[index];
            dyn_scene.replaced_textures.set(index, false);
        }
    }
    ui.table_next_column();
}

fn scalar_gui(ui: &imgui::Ui, name: &str, dyn_scene: &mut rds::DynamicScene, texture_index: u32) {
    let _id = ui.push_id(name);
    let index = texture_index as usize;
    let mut texture = &mut dyn_scene.textures[index];
    let mut bit = dyn_scene.replaced_textures[index];

    ui.text(name);
    ui.table_next_column();

    if let rds::DynamicTexture::Scalar(ref mut value) = &mut texture {
        if imgui::Drag::new("##slider")
            .range(0.0, 1.0)
            .speed(0.01)
            .build(ui, value)
        {
            // Convenience: replace texture when an edit has been made without extra interaction.
            dyn_scene.replaced_textures.set(index, true);
        }
    }
    ui.table_next_column();

    {
        if ui.checkbox("##use", &mut bit) {
            dyn_scene.replaced_textures.set(index, bit);
        }
        ui.same_line();
        if ui.button("X") {
            // Convenience: reset to default value and clear replacement with one click.
            *texture = dyn_scene.default_textures[index];
            dyn_scene.replaced_textures.set(index, false);
        }
    }
    ui.table_next_column();
}
