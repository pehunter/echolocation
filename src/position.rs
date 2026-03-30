use crossterm::event::KeyCode;

pub struct Position(pub f32, pub f32, pub f32);

impl Position {
    pub fn keycode(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Up => {
                self.0 += 0.1;
            }
            KeyCode::Down => {
                self.0 -= 0.1;
            }
            KeyCode::Left => {
                self.1 -= 0.1;
            }
            KeyCode::Right => {
                self.1 += 0.1;
            }
            KeyCode::Char('q') => {
                self.2 += 0.1;
            }
            KeyCode::Char('z') => {
                self.2 -= 0.1;
            }
            _ => {
                return false;
            }
        }

        //Round to tenths
        self.0 = (self.0 * 100.0).round() / 100.0;
        self.1 = (self.1 * 100.0).round() / 100.0;
        self.2 = (self.2 * 100.0).round() / 100.0;

        //Clamp
        self.0 = f32::clamp(-self.limit(), self.limit(), self.0);
        self.1 = f32::clamp(-self.limit(), self.limit(), self.1);
        self.2 = f32::clamp(-self.limit(), self.limit(), self.2);
        return true;
    }

    //Gets the distance to the origin
    pub fn distance(&self) -> f32 {
        return (self.0.powf(2.0) + self.1.powf(2.0) + self.2.powf(2.0)).sqrt();
    }

    //Gets the max bounds for all three axes
    pub fn limit(&self) -> f32 {
        10.0
    }
}
