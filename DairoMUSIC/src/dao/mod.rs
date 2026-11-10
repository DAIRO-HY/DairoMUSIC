#![allow(dead_code)]

// 由build.rs按建表语句和assets/dsl生成的DAO
include!(concat!(env!("OUT_DIR"), "/dao.rs"));
