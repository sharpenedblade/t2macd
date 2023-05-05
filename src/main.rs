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
// along with this program.  If not, see <https://www.gnu.org/licenses/>. 

use glob::glob;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::u32;

struct Fan {
    path: PathBuf,
    max_speed: u32,
    min_speed: u32,
}

impl Fan {
    fn new(path: &PathBuf) -> Fan {
        Fan {
            path: path.clone(),
            max_speed: fs::read_to_string(Path::join(&path, "_max"))
                .expect("Failed to read max speed")
                .parse()
                .expect("Failed to parse max speed"),
            min_speed: fs::read_to_string(Path::join(&path, "_min"))
                .expect("Failed to read min speed")
                .parse()
                .expect("Failed to parse min speed"),
        }
    }

    fn set_speed(&self, speed: u32) {
        let mut output_path: PathBuf = self.path.to_owned();
        output_path.push("_output");
        fs::write(output_path, speed.to_string()).expect("Failed to set fan speed");
    }

    fn set_manual_control(&self, control: bool) {
        let output_path: PathBuf = Path::join(&self.path, "_manual");
        if control {
            fs::write(output_path, "1").expect("Failed to enable manual control");
        } else {
            fs::write(output_path, "0").expect("Failed to disable manual control");
        }
    }
}

fn init_fans() -> Vec<Fan> {
    let mut all_fans = Vec::new();
    // TODO: Remove last 6 chars from file name, currently BROKEN
    for i in glob("/sys/devices/*/*/*/*/APP0001:00/fan*").expect("Failed to locate fan devices") {
        all_fans.push(Fan::new(&i.expect("Error")))
    }
    if all_fans.len() == 0 {
        panic!("Fan objects failed to initialize, there could be no fans");
    }
    return all_fans;
}

fn get_gpu_temp() -> u32 {
    let gpu_temp_path = PathBuf::from("/sys/class/drm/card0/device/hwmon/hwmon*/temp1_input");
    let gpu_temp: String = fs::read_to_string(gpu_temp_path).expect("Failed to get GPU temperature");
    let gpu_temp: u32 = gpu_temp.parse().expect("Failed to parse GPU temperature");
    gpu_temp
}

fn get_cpu_temp() -> u32 {
    let cpu_temp_path = PathBuf::from("/sys/devices/platform/coretemp.0/hwmon/hwmon*/temp1_input");
    let cpu_temp: String = fs::read_to_string(cpu_temp_path).expect("Failed to get CPU temperature");
    let cpu_temp: u32 = cpu_temp.parse().expect("Failed to parse CPU temperature");
    cpu_temp
}

fn get_current_temp() -> u32 {
    if get_gpu_temp() > get_cpu_temp() {
        get_gpu_temp()
    } else {
        get_cpu_temp()
    }
}

fn get_fan_speed_linear(fan: &Fan, current_temp: u32) -> u32 {
    let min_temp: u32 = 80;
    let max_temp: u32 = 100;
    return (current_temp - min_temp) / (max_temp - min_temp) * (fan.max_speed - fan.max_speed)
        + fan.min_speed;
}

fn main() {
    loop {
        let fans = init_fans();
        for fan in fans {
            fan.set_manual_control(true);
            fan.set_speed(get_fan_speed_linear(&fan, get_current_temp()));
        }
    }
}
