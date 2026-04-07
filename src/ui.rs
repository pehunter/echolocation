use std::sync::{Arc, Mutex};

use egui::Ui;

use crate::position::Position;

pub struct Controller<F>
where
    F: FnMut(&Position),
{
    pos: Arc<Mutex<Position>>,
    x: f32,
    y: f32,
    z: f32,
    limit: i32,
    posUpdate: F,
}

impl<F> Controller<F>
where
    F: FnMut(&Position),
{
    pub fn new(pos: Arc<Mutex<Position>>, onPositionUpdate: F) -> Self {
        let (x, y, z, limit);
        {
            let pcap = pos.lock().unwrap();
            (x, y, z, limit) = (pcap.0, pcap.1, pcap.2, pcap.limit());
        }
        Self {
            pos,
            x,
            y,
            z,
            limit: f32::round(limit) as i32,
            posUpdate: onPositionUpdate,
        }
    }
}

impl<F> eframe::App for Controller<F>
where
    F: FnMut(&Position),
{
    fn ui(&mut self, ui: &mut Ui, frame: &mut eframe::Frame) {
        let (px, py, pz) = (self.x, self.y, self.z);
        egui::CentralPanel::default().show_inside(ui, |ui| {
            egui::Grid::new("da grid")
                .num_columns(2)
                .spacing([4.0, 8.0])
                .show(ui, |ui| {
                    ui.heading("Echolocation");
                    ui.end_row();

                    ui.add(egui::Label::new("x (front/back)"));
                    ui.add(
                        egui::DragValue::new(&mut self.x)
                            .speed(0.1)
                            .range(-self.limit..=self.limit),
                    );
                    ui.end_row();

                    ui.add(egui::Label::new("y (left/right)"));
                    ui.add(
                        egui::DragValue::new(&mut self.y)
                            .speed(0.1)
                            .range(-self.limit..=self.limit),
                    );
                    ui.end_row();

                    ui.add(egui::Label::new("z (up/down)"));
                    ui.add(
                        egui::DragValue::new(&mut self.z)
                            .speed(0.1)
                            .range(-self.limit..=self.limit),
                    );
                    ui.end_row();
                })
        });

        //If any value changed, update position
        if px != self.x || py != self.y || pz != self.z {
            let mut pos = self.pos.lock().unwrap();
            pos.0 = self.x;
            pos.1 = self.y;
            pos.2 = self.z;
            (self.posUpdate)(&pos);
        }
    }
}
