use egui::{CollapsingHeader, Widget};
use serde_value::Value;

pub use serde_value::to_value;

pub fn any_inspector(id: u64, mut data: Value, ui: &mut egui::Ui) {
    value_to_gui(ui, &mut data, false, 0, 0, id);
}

pub fn any_editor(id: u64, data: &mut Value, ui: &mut egui::Ui) {
    value_to_gui(ui, data, true, 0, 0, id);
}

pub struct AnyInspector {
    pub id: u64,
    pub data: Value,
}

impl AnyInspector {
    pub fn new(data: Value, id: u64) -> AnyInspector {
        AnyInspector { data, id }
    }
}

impl Widget for &mut AnyInspector {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        value_to_gui(ui, &mut self.data, false, 0, 0, self.id);
        ui.response()
    }
}

macro_rules! numeric_type {
    () => {
        Value::U8(_)
            | Value::U16(_)
            | Value::U32(_)
            | Value::U64(_)
            | Value::I8(_)
            | Value::I16(_)
            | Value::I32(_)
            | Value::I64(_)
            | Value::F32(_)
            | Value::F64(_)
    };
}

fn numeric_editor(ui: &mut egui::Ui, value: &mut Value) {
    match value {
        Value::U8(val) => {
            ui.add(egui::DragValue::new(val));
        }
        Value::U16(val) => {
            ui.add(egui::DragValue::new(val));
        }
        Value::U32(val) => {
            ui.add(egui::DragValue::new(val));
        }
        Value::U64(val) => {
            ui.add(egui::DragValue::new(val));
        }
        Value::I8(val) => {
            ui.add(egui::DragValue::new(val));
        }
        Value::I16(val) => {
            ui.add(egui::DragValue::new(val));
        }
        Value::I32(val) => {
            ui.add(egui::DragValue::new(val));
        }
        Value::I64(val) => {
            ui.add(egui::DragValue::new(val));
        }
        Value::F32(val) => {
            ui.add(egui::DragValue::new(val));
        }
        Value::F64(val) => {
            ui.add(egui::DragValue::new(val));
        }
        _ => panic!(),
    }
}

fn value_to_gui(
    ui: &mut egui::Ui,
    value: &mut Value,
    editable: bool,
    mut depth: u64,
    seq: usize,
    id: u64,
) {
    depth += 1;
    match value {
        Value::Bool(inner) => {
            if editable {
                ui.checkbox(inner, "");
            } else {
                ui.label(inner.to_string());
            }
        }
        Value::Char(inner) => {
            ui.label(inner.to_string());
        }
        numeric_type!() => {
            if editable {
                numeric_editor(ui, value);
            } else {
                ui.label(format!("{value:?}"));
            }
        }
        Value::String(inner) => {
            if editable {
            } else {
                ui.label(inner.to_string());
            }
        }
        Value::Bytes(items) => {
            let mut string = String::new();
            items.iter().for_each(|x| {
                string.push_str(x.to_string().as_str());
                string.push_str(", ");
            });
            ui.label(string);
        }
        Value::Unit => {}
        Value::Option(value) => match value {
            Some(ni) => value_to_gui(ui, ni, editable, depth, 0, id),
            None => (),
        },
        Value::Seq(values) => {
            if values.iter().all(|x| {
                matches!(
                    x,
                    Value::Unit
                        | Value::Bool(_)
                        | numeric_type!()
                        | Value::String(_)
                        | Value::Char(_)
                )
            }) {
                let mut string = String::new();
                values.iter().for_each(|x| {
                    string.push_str(&value_to_string(x));
                    string.push_str(", ");
                });
                ui.label(string);
            } else {
                values.iter_mut().enumerate().for_each(|(index, value)| {
                    ui.label(format!("Entry {index}:"));
                    value_to_gui(ui, value, editable, depth, index, id);
                    ui.separator();
                });
            }
        }
        Value::Map(map) => {
            map.iter_mut()
                .enumerate()
                .for_each(|(index, (name, value))| {
                    map_match(ui, editable, depth, seq, id, index, name, value)
                });
        }
        Value::Newtype(value) => {
            value_to_gui(ui, value, editable, depth, 0, id);
        }
    };
}

fn map_match(
    ui: &mut egui::Ui,
    editable: bool,
    depth: u64,
    seq: usize,
    id: u64,
    index: usize,
    name: &Value,
    value: &mut Value,
) {
    match value {
        Value::Unit
        | Value::Bool(_)
        | numeric_type!()
        | Value::String(_)
        | Value::Char(_)
        | Value::Bytes(_) => {
            ui.horizontal(|ui| {
                ui.label(value_to_string(name));
                value_to_gui(ui, value, editable, depth, 0, id);
            });
        }
        Value::Map(_) | Value::Seq(_) | Value::Option(_) => {
            let name_str = value_to_string(name);
            let id_str = format!("{name_str}{depth}a{index}a{seq}a{id}");
            CollapsingHeader::new(&name_str)
                .id_salt(id_str)
                .show(ui, |ui| {
                    value_to_gui(ui, value, editable, depth, 0, id);
                });
        }
        Value::Newtype(inner) => {
            // Transparently pass the inner value through
            map_match(ui, editable, depth, seq, id, index, name, inner);
        }
    }
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::Bool(inner) => inner.to_string(),
        Value::Char(inner) => inner.to_string(),
        Value::String(inner) => inner.to_owned(),
        Value::Unit => "".to_owned(),
        Value::Option(inner) => match inner {
            Some(ni) => value_to_string(&ni),
            None => "None".to_owned(),
        },
        Value::Newtype(_) | Value::Seq(_) | Value::Map(_) | Value::Bytes(_) | numeric_type!() => {
            format!("{value:?}")
        }
    }
}
