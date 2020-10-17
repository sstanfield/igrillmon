extern crate rumqtt;

use rumqtt::{MqttClient, MqttOptions, Notification, QoS};
use std::fs::File;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};

struct BbqData {
    probe1: u32,
    probe1_last_update: SystemTime,
    probe2: u32,
    probe2_last_update: SystemTime,
    probe3: u32,
    probe3_last_update: SystemTime,
    probe4: u32,
    probe4_last_update: SystemTime,
    battery: u32,
    battery_last_update: SystemTime,
}

fn get_data(data: Arc<Mutex<BbqData>>) {
    let broker = "192.168.1.149";
    let port = 1883;
    let mqtt_options = MqttOptions::new("alarmpi-pubsub1", broker, port).set_keep_alive(10);

    let (mut mqtt_client, notifications) = MqttClient::start(mqtt_options).unwrap();

    mqtt_client
        .subscribe("bbq/probe1", QoS::AtLeastOnce)
        .unwrap();
    mqtt_client
        .subscribe("bbq/probe2", QoS::AtLeastOnce)
        .unwrap();
    mqtt_client
        .subscribe("bbq/probe3", QoS::AtLeastOnce)
        .unwrap();
    mqtt_client
        .subscribe("bbq/probe4", QoS::AtLeastOnce)
        .unwrap();
    mqtt_client
        .subscribe("bbq/battery", QoS::AtLeastOnce)
        .unwrap();

    for notification in notifications {
        //println!("{:?}", notification);
        match notification {
            Notification::Publish(publish) => {
                let payload = std::str::from_utf8(&publish.payload)
                    .unwrap_or("0")
                    .parse::<f32>()
                    .unwrap_or(0.0) as u32;
                let mut data = data.lock().unwrap();

                match &publish.topic_name as &str {
                    "bbq/probe1" => {
                        data.probe1 = payload;
                        data.probe1_last_update = SystemTime::now();
                    }
                    "bbq/probe2" => {
                        data.probe2 = payload;
                        data.probe2_last_update = SystemTime::now();
                    }
                    "bbq/probe3" => {
                        data.probe3 = payload;
                        data.probe3_last_update = SystemTime::now();
                    }
                    "bbq/probe4" => {
                        data.probe4 = payload;
                        data.probe4_last_update = SystemTime::now();
                    }
                    "bbq/battery" => {
                        data.battery = payload;
                        data.battery_last_update = SystemTime::now();
                    }
                    _ => (),
                }
                //009SSTprintln!("{}", payload);
            }
            _ => (),
        }
    }
}

fn handle_read(mut stream: &TcpStream, data: Arc<Mutex<BbqData>>) {
    let mut buf = [0u8; 4096];
    match stream.read(&mut buf) {
        Ok(_) => {
            let req_str = String::from_utf8_lossy(&buf);
            if req_str.to_string().starts_with("GET /data") {
                send_data(stream, data);
            } else {
                handle_write(stream);
            }
        }
        Err(e) => println!("Unable to read stream: {}", e),
    }
}

fn send_data(mut stream: &TcpStream, data: Arc<Mutex<BbqData>>) {
    let now = SystemTime::now();
    let data = data.lock().unwrap();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n\
{{\"probe1\": {}, \"probe1_update_secs\": {},
  \"probe2\": {}, \"probe2_update_secs\": {},
  \"probe3\": {}, \"probe3_update_secs\": {},
  \"probe4\": {}, \"probe4_update_secs\": {},
  \"battery\": {}, \"battery_update_secs\": {}
  }}\
\r\n",
        data.probe1,
        now.duration_since(data.probe1_last_update)
            .unwrap_or(Duration::new(1000, 0))
            .as_secs(),
        data.probe2,
        now.duration_since(data.probe2_last_update)
            .unwrap_or(Duration::new(1000, 0))
            .as_secs(),
        data.probe3,
        now.duration_since(data.probe3_last_update)
            .unwrap_or(Duration::new(1000, 0))
            .as_secs(),
        data.probe4,
        now.duration_since(data.probe4_last_update)
            .unwrap_or(Duration::new(1000, 0))
            .as_secs(),
        data.battery,
        now.duration_since(data.battery_last_update)
            .unwrap_or(Duration::new(1000, 0))
            .as_secs()
    );
    match stream.write(response.as_bytes()) {
        Ok(_) => (),//println!("Data Response sent"),
        Err(e) => println!("Failed sending data response: {}", e),
    }
}

fn handle_write(mut stream: &TcpStream) {
    let mut f = File::open("resources/index.html").expect("index.html not found");

    let mut contents = String::new();
    f.read_to_string(&mut contents).expect("something went wrong reading index.html");

    let response = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=UTF-8\r\n\r\n{}\r\n", contents);
    match stream.write(response.as_bytes()) {
        Ok(_) => (),
        Err(e) => println!("Failed sending response: {}", e),
    }
}

fn handle_client(stream: TcpStream, data: Arc<Mutex<BbqData>>) {
    handle_read(&stream, data);
}

fn main() {
    let now = SystemTime::now();
    let data = Arc::new(Mutex::new(BbqData {
        probe1: 0,
        probe1_last_update: now,
        probe2: 0,
        probe2_last_update: now,
        probe3: 0,
        probe3_last_update: now,
        probe4: 0,
        probe4_last_update: now,
        battery: 0,
        battery_last_update: now,
    }));

    let cdata = data.clone();
    thread::spawn(move || {
        get_data(cdata);
    });
    let listener = TcpListener::bind("0.0.0.0:8050").unwrap();
    println!("Listening for connections on port {}", 8050);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let data = data.clone();
                thread::spawn(|| handle_client(stream, data));
            }
            Err(e) => {
                println!("Unable to connect: {}", e);
            }
        }
    }
}
