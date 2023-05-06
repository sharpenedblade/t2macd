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
use std::process::ExitCode;

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
    fn new(path: PathBuf) -> Result<Fan, std::io::Error> {
        let fan = Fan {
            max_speed: fs::read_to_string(Path::join(&path, "_max"))?
                .parse::<u32>()
                .unwrap(), // This file will always be an int
            min_speed: fs::read_to_string(Path::join(&path, "_min"))?
                .parse::<u32>()
                .unwrap(), // Same as above
            path,
            speed_curve: FanCurve::LINEAR, // TODO: Stop hardcoding fan cruve
        };
        fs::write(Path::join(&fan.path, "_manual"), "1")?;
        return Ok(fan);
    }

    fn set_speed(&self, speed: u32) -> Result<(), std::io::Error> {
        fs::write(Path::join(&self.path, "_output"), speed.to_string())
    }

    fn calc_speed(&self, current_temp: u32) -> u32 {
        let min_temp: u32 = 80; // TODO: Stop hardcoding temp limits
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

fn init_fans() -> Result<Vec<Fan>, std::io::Error> {
    let mut all_fans = Vec::new();
    for i in glob("/sys/devices/*/*/*/*/APP0001:00/fan*_input").unwrap() {
        // TODO: Strip `_input` from file name
        all_fans.push(Fan::new(i.unwrap())?); // Glob is always readable
    }
    return Ok(all_fans);
}

fn get_current_temp() -> u32 {
    // TODO: Resolve glob pattern
    let cpu_temp_path = PathBuf::from("/sys/devices/platform/coretemp.0/hwmon/hwmon*/temp1_input");
    let cpu_temp: String = match fs::read_to_string(cpu_temp_path) {
        Ok(temp) => temp,
        Err(..) => panic!("Failed to read CPU temp. Are you running as root?"),
    };
    let cpu_temp: u32 = cpu_temp.parse::<u32>().unwrap(); // Always parsable

    let gpu_temp_path = PathBuf::from("/sys/class/drm/card0/device/hwmon/hwmon*/temp1_input");
    let gpu_temp: String = match fs::read_to_string(gpu_temp_path) {
        Ok(temp) => temp,
        Err(..) => panic!("Failed to read GPU temp. Are you running as root?"),
    };
    let gpu_temp: u32 = gpu_temp.parse::<u32>().unwrap(); // Same as above

    if gpu_temp > cpu_temp {
        gpu_temp
    } else {
        cpu_temp
    }
}

fn main() -> ExitCode {
    let fans = match init_fans() {
        Ok(fans) => fans,
        Err(..) => panic!("An error occured when initializing fans"),
    };
    if fans.len() == 0 {
        println!("No fans found");
        return ExitCode::from(1);
    }
    loop {
        for fan in &fans {
            match fan.set_speed(fan.calc_speed(get_current_temp())) {
                Ok(..) => continue,
                Err(..) => println!("Error: Failed to set fan speed"),
            }
        }
    }
}
