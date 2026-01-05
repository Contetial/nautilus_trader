//! Generated protocol buffer types
//!
//! This module contains the Rust types generated from .proto files.
//! The actual generated code will be created by build.rs during compilation.

// Include generated broker service types
pub mod broker {
    tonic::include_proto!("nautilus.gateway.broker");
}

// Include generated market data service types
pub mod market_data {
    tonic::include_proto!("nautilus.gateway.market_data");
}

// Include generated order service types
pub mod order {
    tonic::include_proto!("nautilus.gateway.order");
}

// Include generated portfolio service types
pub mod portfolio {
    tonic::include_proto!("nautilus.gateway.portfolio");
}

// Include generated strategy service types
pub mod strategy {
    tonic::include_proto!("nautilus.gateway.strategy");
}
