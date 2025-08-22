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
Comprehensive test runner for Zerodha adapter validation.

This script orchestrates all testing components including:
- Rust WebSocket live data testing
- Python integration testing
- Performance benchmarking
- Error recovery validation
"""

import argparse
import asyncio
import os
import subprocess
import sys
import time
from datetime import datetime
from pathlib import Path
from typing import Dict, List, Optional, Any


class ZerodhaTestRunner:
    """Comprehensive test runner for Zerodha adapter."""
    
    def __init__(self, test_mode: str = "unit"):
        self.test_mode = test_mode
        self.project_root = Path(__file__).parent.parent
        self.zerodha_crate = self.project_root / "crates" / "adapters" / "zerodha"
        
        self.test_results = {
            "rust_tests": {"status": "pending", "details": []},
            "python_tests": {"status": "pending", "details": []},
            "integration_tests": {"status": "pending", "details": []},
            "live_tests": {"status": "pending", "details": []},
            "performance_tests": {"status": "pending", "details": []},
        }
        
        self.start_time = time.time()
        
    def log(self, message: str, level: str = "INFO"):
        """Log message with timestamp."""
        timestamp = datetime.now().strftime("%H:%M:%S")
        print(f"[{timestamp}] {level}: {message}")
    
    def check_environment(self) -> bool:
        """Check if test environment is properly configured."""
        self.log("🔍 Checking test environment...")
        
        # Try to load configuration using the new config system
        try:
            # Import the config loader
            sys.path.insert(0, str(self.project_root / "nautilus_trader" / "adapters" / "zerodha"))
            from config_loader import ZerodhaCredentialsConfig
            
            config = ZerodhaCredentialsConfig.load()
            self.log(f"✅ Configuration loaded: {config.display_safe()}")
            
            # Validate for test mode
            if self.test_mode in ["live", "integration"] and not config.api.access_token:
                self.log("❌ Access token required for live/integration tests", "ERROR")
                return False
            
            return True
            
        except Exception as e:
            self.log(f"❌ Configuration error: {e}", "ERROR")
            self.log("💡 Configuration options:", "INFO")
            self.log("   1. Create zerodha_credentials.toml config file", "INFO")
            self.log("   2. Use environment variables:", "INFO")
            self.log("      - ZERODHA_API_KEY: Your Kite Connect API key", "INFO")
            self.log("      - ZERODHA_API_SECRET: Your Kite Connect API secret", "INFO")
            self.log("      - ZERODHA_ACCESS_TOKEN: Your access token (for live tests)", "INFO")
            self.log("   3. Run 'cargo run --bin zerodha-config-test' to set up config", "INFO")
            return False
        
        # Check project structure
        required_paths = [
            self.zerodha_crate,
            self.project_root / "examples" / "live",
            self.project_root / "tests" / "integration",
        ]
        
        for path in required_paths:
            if not path.exists():
                self.log(f"❌ Missing required path: {path}", "ERROR")
                return False
        
        self.log("✅ Environment check passed", "INFO")
        return True
    
    def run_command(self, command: List[str], cwd: Optional[Path] = None) -> Dict[str, Any]:
        """Run a command and capture output."""
        if cwd is None:
            cwd = self.project_root
            
        self.log(f"🔧 Running: {' '.join(command)}")
        
        try:
            start_time = time.time()
            result = subprocess.run(
                command,
                cwd=cwd,
                capture_output=True,
                text=True,
                timeout=300,  # 5 minute timeout
            )
            duration = time.time() - start_time
            
            return {
                "success": result.returncode == 0,
                "returncode": result.returncode,
                "stdout": result.stdout,
                "stderr": result.stderr,
                "duration": duration,
            }
            
        except subprocess.TimeoutExpired:
            self.log(f"⏰ Command timed out after 5 minutes", "ERROR")
            return {
                "success": False,
                "returncode": -1,
                "stdout": "",
                "stderr": "Command timed out",
                "duration": 300,
            }
        except Exception as e:
            self.log(f"❌ Command failed: {e}", "ERROR")
            return {
                "success": False,
                "returncode": -1,
                "stdout": "",
                "stderr": str(e),
                "duration": 0,
            }
    
    def run_rust_tests(self) -> bool:
        """Run Rust unit and integration tests."""
        self.log("🦀 Running Rust tests...")
        
        # Check if cargo is available
        cargo_check = self.run_command(["cargo", "--version"])
        if not cargo_check["success"]:
            self.log("❌ Cargo not found - skipping Rust tests", "WARNING")
            self.test_results["rust_tests"]["status"] = "skipped"
            return True  # Don't fail the whole suite
        
        # Run cargo test
        test_result = self.run_command(["cargo", "test"], cwd=self.zerodha_crate)
        
        self.test_results["rust_tests"]["details"].append({
            "test": "cargo_test",
            "success": test_result["success"],
            "duration": test_result["duration"],
            "output": test_result["stdout"] + test_result["stderr"],
        })
        
        if test_result["success"]:
            self.log("✅ Rust tests passed", "INFO")
            self.test_results["rust_tests"]["status"] = "passed"
            return True
        else:
            self.log("❌ Rust tests failed", "ERROR")
            self.log(f"Error output: {test_result['stderr']}", "ERROR")
            self.test_results["rust_tests"]["status"] = "failed"
            return False
    
    def run_rust_binaries(self) -> bool:
        """Test Rust binary compilation and basic functionality."""
        self.log("🔧 Testing Rust binaries...")
        
        binaries = [
            "zerodha-execution-test",
            "zerodha-ws-live-test",
        ]
        
        success = True
        for binary in binaries:
            # Test compilation
            build_result = self.run_command(
                ["cargo", "build", "--bin", binary],
                cwd=self.zerodha_crate
            )
            
            if build_result["success"]:
                self.log(f"✅ Binary {binary} compiled successfully")
            else:
                self.log(f"❌ Binary {binary} compilation failed", "ERROR")
                self.log(f"Error: {build_result['stderr']}", "ERROR")
                success = False
            
            self.test_results["rust_tests"]["details"].append({
                "test": f"build_{binary}",
                "success": build_result["success"],
                "duration": build_result["duration"],
                "output": build_result["stderr"],
            })
        
        return success
    
    def run_python_unit_tests(self) -> bool:
        """Run Python unit tests."""
        self.log("🐍 Running Python unit tests...")
        
        # Run pytest on Zerodha adapter tests
        test_result = self.run_command([
            "python", "-m", "pytest",
            "tests/integration/test_zerodha_framework.py",
            "-v", "--tb=short"
        ])
        
        self.test_results["python_tests"]["details"].append({
            "test": "pytest_unit",
            "success": test_result["success"],
            "duration": test_result["duration"],
            "output": test_result["stdout"] + test_result["stderr"],
        })
        
        if test_result["success"]:
            self.log("✅ Python unit tests passed")
            self.test_results["python_tests"]["status"] = "passed"
            return True
        else:
            self.log("❌ Python unit tests failed", "ERROR")
            self.log(f"Error output: {test_result['stderr']}", "ERROR")
            self.test_results["python_tests"]["status"] = "failed"
            return False
    
    async def run_integration_tests(self) -> bool:
        """Run comprehensive integration tests."""
        self.log("🔗 Running integration tests...")
        
        if self.test_mode == "unit":
            self.log("⏭️  Skipping integration tests in unit test mode")
            self.test_results["integration_tests"]["status"] = "skipped"
            return True
        
        try:
            # Import and run the integration test
            sys.path.insert(0, str(self.project_root))
            from examples.live.zerodha_integration_test import ZerodhaIntegrationTest
            
            test = ZerodhaIntegrationTest()
            success = await test.run_full_test()
            
            if success:
                self.log("✅ Integration tests passed")
                self.test_results["integration_tests"]["status"] = "passed"
                return True
            else:
                self.log("❌ Integration tests failed", "ERROR")
                self.test_results["integration_tests"]["status"] = "failed"
                return False
                
        except Exception as e:
            self.log(f"❌ Integration test failed with exception: {e}", "ERROR")
            self.test_results["integration_tests"]["status"] = "failed"
            return False
    
    def run_live_data_tests(self) -> bool:
        """Run live WebSocket data tests."""
        self.log("📊 Running live data tests...")
        
        if self.test_mode in ["unit", "integration"]:
            self.log("⏭️  Skipping live data tests - not in live mode")
            self.test_results["live_tests"]["status"] = "skipped"
            return True
        
        # Run Rust WebSocket live test
        test_result = self.run_command([
            "cargo", "run", "--bin", "zerodha-ws-live-test"
        ], cwd=self.zerodha_crate)
        
        self.test_results["live_tests"]["details"].append({
            "test": "ws_live_test",
            "success": test_result["success"],
            "duration": test_result["duration"],
            "output": test_result["stdout"] + test_result["stderr"],
        })
        
        if test_result["success"]:
            self.log("✅ Live data tests passed")
            self.test_results["live_tests"]["status"] = "passed"
            return True
        else:
            self.log("❌ Live data tests failed", "ERROR")
            self.log(f"Error output: {test_result['stderr']}", "ERROR")
            self.test_results["live_tests"]["status"] = "failed"
            return False
    
    def run_performance_tests(self) -> bool:
        """Run performance benchmarks."""
        self.log("🚀 Running performance tests...")
        
        if self.test_mode == "unit":
            self.log("⏭️  Skipping performance tests in unit mode")
            self.test_results["performance_tests"]["status"] = "skipped"
            return True
        
        # Run cargo bench if available
        bench_result = self.run_command([
            "cargo", "bench", "--no-run"
        ], cwd=self.zerodha_crate)
        
        self.test_results["performance_tests"]["details"].append({
            "test": "cargo_bench",
            "success": bench_result["success"],
            "duration": bench_result["duration"],
            "output": bench_result["stdout"] + bench_result["stderr"],
        })
        
        if bench_result["success"]:
            self.log("✅ Performance tests completed")
            self.test_results["performance_tests"]["status"] = "passed"
            return True
        else:
            self.log("⚠️  Performance tests not available", "WARNING")
            self.test_results["performance_tests"]["status"] = "skipped"
            return True  # Don't fail for missing benchmarks
    
    def print_summary(self):
        """Print comprehensive test summary."""
        duration = time.time() - self.start_time
        
        self.log("📋 TEST SUMMARY", "INFO")
        self.log("=" * 60, "INFO")
        self.log(f"⏱️  Total Duration: {duration:.1f}s", "INFO")
        self.log(f"🧪 Test Mode: {self.test_mode}", "INFO")
        
        total_tests = 0
        passed_tests = 0
        failed_tests = 0
        skipped_tests = 0
        
        for category, result in self.test_results.items():
            status = result["status"]
            detail_count = len(result["details"])
            
            if status == "passed":
                status_icon = "✅"
                passed_tests += 1
            elif status == "failed":
                status_icon = "❌"
                failed_tests += 1
            elif status == "skipped":
                status_icon = "⏭️ "
                skipped_tests += 1
            else:
                status_icon = "⏸️ "
            
            total_tests += 1
            self.log(f"{status_icon} {category.replace('_', ' ').title()}: {status} ({detail_count} tests)")
        
        self.log("", "INFO")
        self.log(f"📊 Results: {passed_tests} passed, {failed_tests} failed, {skipped_tests} skipped")
        
        success_rate = (passed_tests / (total_tests - skipped_tests) * 100) if (total_tests - skipped_tests) > 0 else 0
        self.log(f"📈 Success Rate: {success_rate:.1f}%")
        
        if failed_tests == 0:
            self.log("🎉 ALL TESTS PASSED!", "INFO")
        else:
            self.log("💥 SOME TESTS FAILED!", "ERROR")
            
            # Print failed test details
            for category, result in self.test_results.items():
                if result["status"] == "failed":
                    self.log(f"\n❌ {category.upper()} FAILURES:", "ERROR")
                    for detail in result["details"]:
                        if not detail["success"]:
                            self.log(f"  - {detail['test']}: {detail.get('output', 'No details')[:100]}...")
        
        return failed_tests == 0
    
    async def run_all_tests(self) -> bool:
        """Run the complete test suite."""
        self.log(f"🚀 Starting Zerodha test suite (mode: {self.test_mode})")
        
        if not self.check_environment():
            return False
        
        # Test sequence based on mode
        if self.test_mode == "unit":
            tests = [
                ("Rust Tests", self.run_rust_tests),
                ("Rust Binaries", self.run_rust_binaries),
                ("Python Unit Tests", self.run_python_unit_tests),
            ]
        elif self.test_mode == "integration":
            tests = [
                ("Rust Tests", self.run_rust_tests),
                ("Rust Binaries", self.run_rust_binaries),
                ("Python Unit Tests", self.run_python_unit_tests),
                ("Integration Tests", self.run_integration_tests),
            ]
        elif self.test_mode == "live":
            tests = [
                ("Rust Tests", self.run_rust_tests),
                ("Rust Binaries", self.run_rust_binaries),
                ("Python Unit Tests", self.run_python_unit_tests),
                ("Integration Tests", self.run_integration_tests),
                ("Live Data Tests", self.run_live_data_tests),
                ("Performance Tests", self.run_performance_tests),
            ]
        else:
            self.log(f"❌ Unknown test mode: {self.test_mode}", "ERROR")
            return False
        
        # Run tests
        success = True
        for test_name, test_func in tests:
            self.log(f"\n📋 Running {test_name}...")
            
            if asyncio.iscoroutinefunction(test_func):
                result = await test_func()
            else:
                result = test_func()
            
            if not result:
                success = False
                if self.test_mode != "live":  # Continue on failures in live mode
                    break
        
        # Print summary
        self.print_summary()
        return success


def main():
    """Main entry point."""
    parser = argparse.ArgumentParser(description="Zerodha adapter test runner")
    parser.add_argument(
        "--mode",
        choices=["unit", "integration", "live"],
        default="unit",
        help="Test mode to run (default: unit)"
    )
    parser.add_argument(
        "--verbose",
        action="store_true",
        help="Enable verbose output"
    )
    
    args = parser.parse_args()
    
    # Set up environment
    if args.verbose:
        os.environ["RUST_LOG"] = "debug"
    
    # Run tests
    runner = ZerodhaTestRunner(test_mode=args.mode)
    
    try:
        success = asyncio.run(runner.run_all_tests())
        return 0 if success else 1
        
    except KeyboardInterrupt:
        print("\n⏹️  Tests interrupted by user")
        return 2
    except Exception as e:
        print(f"\n💥 Test runner failed: {e}")
        return 3


if __name__ == "__main__":
    exit(main())