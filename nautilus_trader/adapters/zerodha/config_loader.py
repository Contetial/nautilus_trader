#!/usr/bin/env python3
# -------------------------------------------------------------------------------------------------
#  Copyright (C) 2015-2025 Nautech Systems Pty Ltd. All rights reserved.
#  https://nautechsystems.io
#
#  Licensed under the GNU Lesser General Public License Version 3.0 (the "License");
#  You may not use this file except in compliance with the License.
#  You may obtain a copy of the License at https://www.gnu.org/licenses/lgpl-3.0.en.html
#
#  Unless required by applicable law or agreed to in writing, software
#  distributed under the License is distributed on an "AS IS" BASIS,
#  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
#  See the License for the specific language governing permissions and
#  limitations under the License.
# -------------------------------------------------------------------------------------------------

"""
Configuration loader for Zerodha adapter.

This module provides secure loading and management of Zerodha API credentials
from configuration files with fallback to environment variables.
"""

import os
import logging
from pathlib import Path
from typing import Dict, List, Optional, Any
from dataclasses import dataclass, asdict

try:
    import toml
except ImportError:
    print("ERROR: toml package not found. Install with: pip install toml")
    raise


@dataclass
class ApiCredentials:
    """API credentials configuration."""
    api_key: str
    api_secret: str
    access_token: Optional[str] = None


@dataclass
class GeneralSettings:
    """General settings configuration."""
    trading_mode: str = "paper"  # "live" or "paper"
    sandbox: bool = True
    default_product_type: str = "MIS"
    default_validity: str = "DAY"


@dataclass
class PaperTradingConfig:
    """Paper trading configuration."""
    initial_balance: float = 1_000_000.0
    commission_per_trade: float = 20.0
    execution_delay_ms: int = 100
    simulate_market_data: bool = True


@dataclass
class RiskManagementConfig:
    """Risk management configuration."""
    max_order_value: float = 100_000.0
    max_position_value: float = 1_000_000.0
    max_orders_per_minute: int = 10
    enable_validation: bool = True


@dataclass
class LoggingConfig:
    """Logging configuration."""
    log_level: str = "INFO"
    log_to_file: bool = True
    log_file: str = "logs/zerodha_adapter.log"


@dataclass
class TestingConfig:
    """Testing configuration."""
    test_duration: int = 300
    test_instruments: List[str] = None
    min_data_quality: float = 85.0
    max_error_rate: float = 5.0
    
    def __post_init__(self):
        if self.test_instruments is None:
            self.test_instruments = [
                "RELIANCE", "TCS", "HDFCBANK", "INFY", "ICICIBANK"
            ]


@dataclass
class ZerodhaCredentialsConfig:
    """Complete Zerodha configuration including credentials and settings."""
    api: ApiCredentials
    settings: GeneralSettings = None
    paper_trading: PaperTradingConfig = None
    risk_management: RiskManagementConfig = None
    logging: LoggingConfig = None
    testing: TestingConfig = None
    
    def __post_init__(self):
        if self.settings is None:
            self.settings = GeneralSettings()
        if self.paper_trading is None:
            self.paper_trading = PaperTradingConfig()
        if self.risk_management is None:
            self.risk_management = RiskManagementConfig()
        if self.logging is None:
            self.logging = LoggingConfig()
        if self.testing is None:
            self.testing = TestingConfig()
    
    @classmethod
    def load(cls) -> 'ZerodhaCredentialsConfig':
        """Load configuration from file with fallbacks."""
        logger = logging.getLogger(__name__)
        
        # Try to load from various possible locations
        config_paths = cls._get_config_paths()
        
        for path in config_paths:
            if path.exists():
                logger.info(f"Loading Zerodha config from: {path}")
                return cls._load_from_file(path)
        
        # If no config file found, try environment variables
        logger.warning("No config file found, attempting to load from environment variables")
        return cls._load_from_env()
    
    @classmethod
    def _load_from_file(cls, path: Path) -> 'ZerodhaCredentialsConfig':
        """Load configuration from specific file."""
        logger = logging.getLogger(__name__)
        
        try:
            with open(path, 'r') as f:
                data = toml.load(f)
            
            # Create configuration from TOML data
            config = cls._from_dict(data)
            
            # Override with environment variables if they exist
            config._apply_env_overrides()
            
            # Validate configuration
            config.validate()
            
            logger.info(f"Configuration loaded successfully from {path}")
            return config
            
        except Exception as e:
            raise ValueError(f"Failed to load config from {path}: {e}")
    
    @classmethod
    def _load_from_env(cls) -> 'ZerodhaCredentialsConfig':
        """Load configuration from environment variables."""
        logger = logging.getLogger(__name__)
        
        # Load API credentials from environment
        api_key = os.getenv("ZERODHA_API_KEY")
        if not api_key:
            raise ValueError("ZERODHA_API_KEY environment variable not set")
        
        api_secret = os.getenv("ZERODHA_API_SECRET")
        if not api_secret:
            raise ValueError("ZERODHA_API_SECRET environment variable not set")
        
        access_token = os.getenv("ZERODHA_ACCESS_TOKEN")
        
        api_creds = ApiCredentials(
            api_key=api_key,
            api_secret=api_secret,
            access_token=access_token
        )
        
        config = cls(api=api_creds)
        
        # Optional overrides
        if trading_mode := os.getenv("ZERODHA_TRADING_MODE"):
            config.settings.trading_mode = trading_mode
        
        if sandbox := os.getenv("ZERODHA_SANDBOX"):
            config.settings.sandbox = sandbox.lower() == "true"
        
        config.validate()
        
        logger.info("Configuration loaded from environment variables")
        return config
    
    @classmethod
    def _from_dict(cls, data: Dict[str, Any]) -> 'ZerodhaCredentialsConfig':
        """Create configuration from dictionary."""
        api_data = data.get("api", {})
        api = ApiCredentials(**api_data)
        
        settings_data = data.get("settings", {})
        settings = GeneralSettings(**settings_data)
        
        paper_trading_data = data.get("paper_trading", {})
        paper_trading = PaperTradingConfig(**paper_trading_data)
        
        risk_management_data = data.get("risk_management", {})
        risk_management = RiskManagementConfig(**risk_management_data)
        
        logging_data = data.get("logging", {})
        logging_config = LoggingConfig(**logging_data)
        
        testing_data = data.get("testing", {})
        testing = TestingConfig(**testing_data)
        
        return cls(
            api=api,
            settings=settings,
            paper_trading=paper_trading,
            risk_management=risk_management,
            logging=logging_config,
            testing=testing
        )
    
    def _apply_env_overrides(self):
        """Apply environment variable overrides to loaded config."""
        logger = logging.getLogger(__name__)
        
        # API credentials overrides
        if api_key := os.getenv("ZERODHA_API_KEY"):
            logger.debug("Overriding API key from environment")
            self.api.api_key = api_key
        
        if api_secret := os.getenv("ZERODHA_API_SECRET"):
            logger.debug("Overriding API secret from environment")
            self.api.api_secret = api_secret
        
        if access_token := os.getenv("ZERODHA_ACCESS_TOKEN"):
            logger.debug("Overriding access token from environment")
            self.api.access_token = access_token
        
        # Settings overrides
        if trading_mode := os.getenv("ZERODHA_TRADING_MODE"):
            logger.debug(f"Overriding trading mode from environment: {trading_mode}")
            self.settings.trading_mode = trading_mode
        
        if sandbox := os.getenv("ZERODHA_SANDBOX"):
            sandbox_bool = sandbox.lower() == "true"
            logger.debug(f"Overriding sandbox mode from environment: {sandbox_bool}")
            self.settings.sandbox = sandbox_bool
        
        # Logging overrides
        if log_level := os.getenv("RUST_LOG"):
            logger.debug(f"Overriding log level from environment: {log_level}")
            self.logging.log_level = log_level.upper()
    
    def validate(self):
        """Validate configuration."""
        # Validate API credentials
        if not self.api.api_key:
            raise ValueError("API key cannot be empty")
        
        if not self.api.api_secret:
            raise ValueError("API secret cannot be empty")
        
        # Validate trading mode
        if self.settings.trading_mode not in ["live", "paper"]:
            raise ValueError("Trading mode must be 'live' or 'paper'")
        
        # Validate paper trading settings
        if self.paper_trading.initial_balance <= 0:
            raise ValueError("Paper trading initial balance must be positive")
        
        # Validate risk management settings
        if self.risk_management.max_order_value <= 0:
            raise ValueError("Maximum order value must be positive")
        
        if self.risk_management.max_position_value <= 0:
            raise ValueError("Maximum position value must be positive")
        
        # Validate testing settings
        if not 0 <= self.testing.min_data_quality <= 100:
            raise ValueError("Minimum data quality must be between 0 and 100")
        
        if not 0 <= self.testing.max_error_rate <= 100:
            raise ValueError("Maximum error rate must be between 0 and 100")
    
    @staticmethod
    def _get_config_paths() -> List[Path]:
        """Get possible configuration file paths."""
        paths = []
        
        # Current directory
        paths.append(Path("zerodha_credentials.toml"))
        paths.append(Path("config/zerodha_credentials.toml"))
        
        # Project root (assuming we're in nautilus_trader directory)
        paths.append(Path("../config/zerodha_credentials.toml"))
        paths.append(Path("../../config/zerodha_credentials.toml"))
        
        # Home directory
        home_dir = Path.home()
        paths.append(home_dir / ".zerodha_credentials.toml")
        paths.append(home_dir / ".config" / "zerodha_credentials.toml")
        
        return paths
    
    def save_to_file(self, path: Path):
        """Save configuration to file."""
        # Create directory if it doesn't exist
        path.parent.mkdir(parents=True, exist_ok=True)
        
        # Convert to dictionary
        config_dict = {
            "api": asdict(self.api),
            "settings": asdict(self.settings),
            "paper_trading": asdict(self.paper_trading),
            "risk_management": asdict(self.risk_management),
            "logging": asdict(self.logging),
            "testing": asdict(self.testing),
        }
        
        # Save to file
        with open(path, 'w') as f:
            toml.dump(config_dict, f)
        
        logging.getLogger(__name__).info(f"Configuration saved to {path}")
    
    def is_paper_trading(self) -> bool:
        """Check if running in paper trading mode."""
        return self.settings.trading_mode == "paper"
    
    def is_sandbox(self) -> bool:
        """Check if running in sandbox mode."""
        return self.settings.sandbox
    
    def display_safe(self) -> str:
        """Get display-safe version of config (hides sensitive data)."""
        api_key_safe = f"{self.api.api_key[:8]}***" if len(self.api.api_key) > 8 else "***"
        
        return (
            f"ZerodhaConfig {{ "
            f"api_key: {api_key_safe}, "
            f"trading_mode: {self.settings.trading_mode}, "
            f"sandbox: {self.settings.sandbox}, "
            f"paper_balance: Rs.{self.paper_trading.initial_balance:.2f}, "
            f"max_order_value: Rs.{self.risk_management.max_order_value:.2f} "
            f"}}"
        )


class ZerodhaCredentialsBuilder:
    """Builder for programmatic configuration setup."""
    
    def __init__(self):
        self.api_key = ""
        self.api_secret = ""
        self.access_token = None
        self.trading_mode = "paper"
        self.sandbox = True
        self.paper_balance = 1_000_000.0
        self.max_order_value = 100_000.0
    
    def with_api_credentials(self, api_key: str, api_secret: str, access_token: Optional[str] = None):
        """Set API credentials."""
        self.api_key = api_key
        self.api_secret = api_secret
        self.access_token = access_token
        return self
    
    def with_trading_mode(self, mode: str):
        """Set trading mode."""
        self.trading_mode = mode
        return self
    
    def with_sandbox(self, sandbox: bool):
        """Enable/disable sandbox mode."""
        self.sandbox = sandbox
        return self
    
    def with_paper_balance(self, balance: float):
        """Set paper trading balance."""
        self.paper_balance = balance
        return self
    
    def with_max_order_value(self, value: float):
        """Set maximum order value."""
        self.max_order_value = value
        return self
    
    def build(self) -> ZerodhaCredentialsConfig:
        """Build and validate configuration."""
        api = ApiCredentials(
            api_key=self.api_key,
            api_secret=self.api_secret,
            access_token=self.access_token
        )
        
        settings = GeneralSettings(
            trading_mode=self.trading_mode,
            sandbox=self.sandbox
        )
        
        paper_trading = PaperTradingConfig(
            initial_balance=self.paper_balance
        )
        
        risk_management = RiskManagementConfig(
            max_order_value=self.max_order_value
        )
        
        config = ZerodhaCredentialsConfig(
            api=api,
            settings=settings,
            paper_trading=paper_trading,
            risk_management=risk_management
        )
        
        config.validate()
        return config


def create_example_config_file(path: Path = None) -> Path:
    """Create an example configuration file."""
    if path is None:
        path = Path("zerodha_credentials.toml")
    
    builder = ZerodhaCredentialsBuilder()
    example_config = builder.with_api_credentials(
        "your_api_key_here",
        "your_api_secret_here", 
        "your_access_token_here"
    ).build()
    
    example_config.save_to_file(path)
    return path


# Example usage
if __name__ == "__main__":
    logging.basicConfig(level=logging.INFO)
    
    print("Zerodha Configuration Loader Test")
    print("====================================")
    
    try:
        # Try to load configuration
        config = ZerodhaCredentialsConfig.load()
        print(f"Configuration loaded: {config.display_safe()}")
        
    except Exception as e:
        print(f"Failed to load configuration: {e}")
        print("Creating example configuration file...")
        
        example_path = create_example_config_file()
        print(f"Example configuration created: {example_path}")
        print("Edit the file and replace placeholder values with your actual credentials")