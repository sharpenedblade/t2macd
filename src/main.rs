// Copyright (C) 2023 t2macd contributors
// SPDX-License-Identifier: GPL-3.0-or-later
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use glob::glob;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::u32;

enum FanCurve {
    LINEAR,
}

struct Fan {
    path: PathBuf,
    max_speed: u32,
    min_speed: u32,
    speed_curve: FanCurve,
}

impl Fan {
    fn new(path: PathBuf) -> Fan {
        let fan = Fan {
            max_speed: fs::read_to_string(Path::join(&path, "_max"))
                .expect("Failed to read max speed")
                .parse()
                .expect("Failed to parse max speed"),
            min_speed: fs::read_to_string(Path::join(&path, "_min"))
                .expect("Failed to read min speed")
                .parse()
                .expect("Failed to parse min speed"),
            path,
            speed_curve: FanCurve::LINEAR,
        };
        fs::write(Path::join(&fan.path, "_manual"), "1")
            .expect("Failed to enable manual fan control");
        return fan;
    }

    fn set_speed(&self, speed: u32) {
        fs::write(Path::join(&self.path, "_output"), speed.to_string())
            .expect("Failed to set fan speed");
    }

    fn calc_speed(&self, current_temp: u32) -> u32 {
        let min_temp: u32 = 80;
        let max_temp: u32 = 100;
        match self.speed_curve {
            FanCurve::LINEAR => {
                (current_temp - min_temp) / (max_temp - min_temp)
                    * (self.max_speed - self.max_speed)
                    + self.min_speed
            }
        }
    }
}

fn init_fans() -> Vec<Fan> {
    let mut all_fans = Vec::new();
    // TODO: Remove "_input" from file name, currently BROKEN
    for i in glob("/sys/devices/*/*/*/*/APP0001:00/fan*").expect("Failed to locate fan") {
        all_fans.push(Fan::new(i.expect("Error")))
    }
    if all_fans.len() == 0 {
        panic!("Fan objects failed to initialize");
    }
    return all_fans;
}

fn get_current_temp() -> u32 {
    let cpu_temp_path = PathBuf::from("/sys/devices/platform/coretemp.0/hwmon/hwmon*/temp1_input");
    let cpu_temp: String =
        fs::read_to_string(cpu_temp_path).expect("Failed to get CPU temperature");
    let cpu_temp: u32 = cpu_temp.parse().expect("Failed to parse CPU temperature");

    let gpu_temp_path = PathBuf::from("/sys/class/drm/card0/device/hwmon/hwmon*/temp1_input");
    let gpu_temp: String =
        fs::read_to_string(gpu_temp_path).expect("Failed to get GPU temperature");
    let gpu_temp: u32 = gpu_temp.parse().expect("Failed to parse GPU temperature");

    if gpu_temp > cpu_temp {
        gpu_temp
    } else {
        cpu_temp
    }
}

fn main() {
    let fans = init_fans();
    loop {
        for fan in &fans {
            fan.set_speed(fan.calc_speed(get_current_temp()));
        }
    }
}
