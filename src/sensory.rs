use crate::{Hypervector, VarConfig};
use std::collections::HashMap;

pub trait SensoryModality: Send + Sync {
    fn encode(&self) -> Hypervector;
    fn name(&self) -> &str;
}

pub struct TextSensoryModality {
    pub text: String,
    pub name: String,
}

impl TextSensoryModality {
    pub fn new(name: &str, text: &str) -> Self {
        TextSensoryModality {
            text: text.to_string(),
            name: name.to_string(),
        }
    }
}

impl SensoryModality for TextSensoryModality {
    fn encode(&self) -> Hypervector {
        Hypervector::encode_sentence(&self.text)
    }

    fn name(&self) -> &str {
        &self.name
    }
}

pub struct SystemTelemetryModality {
    pub name: String,
    pub variables: HashMap<String, VarConfig>,
    pub readings: HashMap<String, f64>,
}

impl SystemTelemetryModality {
    pub fn new(name: &str) -> Self {
        let mut variables = HashMap::new();
        // Register CPU (0 to 100)
        variables.insert(
            "cpu_utilization".to_string(),
            VarConfig {
                id: Hypervector::new_random(),
                min_val: 0.0,
                max_val: 100.0,
                base_min: Hypervector::new_random(),
                base_max: Hypervector::new_random(),
            },
        );
        // Register RAM Free (0 to 64GB)
        variables.insert(
            "ram_free_gb".to_string(),
            VarConfig {
                id: Hypervector::new_random(),
                min_val: 0.0,
                max_val: 64.0,
                base_min: Hypervector::new_random(),
                base_max: Hypervector::new_random(),
            },
        );

        SystemTelemetryModality {
            name: name.to_string(),
            variables,
            readings: HashMap::new(),
        }
    }

    pub fn set_reading(&mut self, key: &str, value: f64) {
        self.readings.insert(key.to_string(), value);
    }
}

impl SensoryModality for SystemTelemetryModality {
    fn encode(&self) -> Hypervector {
        let mut bound_vectors = Vec::new();
        for (key, config) in &self.variables {
            let val = self.readings.get(key).cloned().unwrap_or(config.min_val);
            let encoded_val = Hypervector::encode_continuous(config, val);
            bound_vectors.push(config.id.bitwise_xor(&encoded_val));
        }
        let refs: Vec<&Hypervector> = bound_vectors.iter().collect();
        Hypervector::bundle(&refs)
    }

    fn name(&self) -> &str {
        &self.name
    }
}

pub struct NetworkTrafficModality {
    pub name: String,
    pub active_connections: usize,
    pub bandwidth_mbps: f64,
    pub conn_config: VarConfig,
    pub bw_config: VarConfig,
}

impl NetworkTrafficModality {
    pub fn new(name: &str) -> Self {
        NetworkTrafficModality {
            name: name.to_string(),
            active_connections: 0,
            bandwidth_mbps: 0.0,
            conn_config: VarConfig {
                id: Hypervector::new_random(),
                min_val: 0.0,
                max_val: 1000.0,
                base_min: Hypervector::new_random(),
                base_max: Hypervector::new_random(),
            },
            bw_config: VarConfig {
                id: Hypervector::new_random(),
                min_val: 0.0,
                max_val: 10000.0,
                base_min: Hypervector::new_random(),
                base_max: Hypervector::new_random(),
            },
        }
    }
}

impl SensoryModality for NetworkTrafficModality {
    fn encode(&self) -> Hypervector {
        let conn_vec =
            Hypervector::encode_continuous(&self.conn_config, self.active_connections as f64);
        let bound_conn = self.conn_config.id.bitwise_xor(&conn_vec);

        let bw_vec = Hypervector::encode_continuous(&self.bw_config, self.bandwidth_mbps);
        let bound_bw = self.bw_config.id.bitwise_xor(&bw_vec);

        Hypervector::bundle(&[&bound_conn, &bound_bw])
    }

    fn name(&self) -> &str {
        &self.name
    }
}
