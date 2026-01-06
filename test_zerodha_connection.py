#!/usr/bin/env python3
"""Quick test to verify Zerodha API connectivity."""

import requests
import sys
from pathlib import Path

# Credentials from config
API_KEY = "7zks6tfai2vje1at"
API_SECRET = "gt8hvnuiwtz7w8a3dwd8tcqnvguk31el"
ACCESS_TOKEN = "I76VcmH33rx5vsRTU03C69XgFLCtbeST"

def test_connection():
    """Test basic API connectivity."""
    print("=" * 60)
    print("ZERODHA API CONNECTION TEST")
    print("=" * 60)
    print()
    print(f"API Key: {API_KEY[:8]}***")
    print(f"Access Token: {ACCESS_TOKEN[:8]}***")
    print()

    headers = {
        "X-Kite-Version": "3",
        "Authorization": f"token {API_KEY}:{ACCESS_TOKEN}"
    }

    # Test 1: User Profile
    print("Test 1: Fetching user profile...")
    try:
        response = requests.get(
            "https://api.kite.trade/user/profile",
            headers=headers,
            timeout=30
        )

        if response.status_code == 200:
            data = response.json()
            if data.get("status") == "success":
                user = data["data"]
                print(f"  SUCCESS! Connected as: {user.get('user_name')} ({user.get('user_id')})")
                print(f"  Email: {user.get('email')}")
                print(f"  Broker: {user.get('broker')}")
                print(f"  Exchanges: {', '.join(user.get('exchanges', []))}")
            else:
                print(f"  API returned error: {data}")
                return False
        elif response.status_code == 403:
            print("  TOKEN EXPIRED or INVALID!")
            print("  You need to generate a new access token.")
            print()
            show_token_instructions()
            return False
        else:
            print(f"  HTTP {response.status_code}: {response.text}")
            return False

    except Exception as e:
        print(f"  ERROR: {e}")
        return False

    print()

    # Test 2: Fetch instruments (just count)
    print("Test 2: Fetching instruments...")
    try:
        response = requests.get(
            "https://api.kite.trade/instruments",
            headers=headers,
            timeout=60
        )

        if response.status_code == 200:
            lines = response.text.strip().split('\n')
            instrument_count = len(lines) - 1  # Subtract header
            print(f"  SUCCESS! Found {instrument_count:,} instruments")

            # Show some sample instruments
            if len(lines) > 1:
                print("  Sample instruments (first 5 equities):")
                count = 0
                for line in lines[1:]:  # Skip header
                    fields = line.split(',')
                    if len(fields) >= 12 and fields[9] == "EQ" and fields[11] == "NSE":
                        print(f"    - {fields[2]} ({fields[3][:30]})")
                        count += 1
                        if count >= 5:
                            break
        else:
            print(f"  HTTP {response.status_code}: {response.text[:200]}")
            return False

    except Exception as e:
        print(f"  ERROR: {e}")
        return False

    print()

    # Test 3: Fetch margins
    print("Test 3: Fetching account margins...")
    try:
        response = requests.get(
            "https://api.kite.trade/user/margins",
            headers=headers,
            timeout=30
        )

        if response.status_code == 200:
            data = response.json()
            if data.get("status") == "success":
                equity = data["data"].get("equity", {})
                available = equity.get("available", {})
                print(f"  SUCCESS!")
                print(f"  Available Cash: Rs. {available.get('cash', 0):,.2f}")
                print(f"  Available Margin: Rs. {available.get('live_balance', 0):,.2f}")
        else:
            print(f"  HTTP {response.status_code}")

    except Exception as e:
        print(f"  ERROR: {e}")

    print()
    print("=" * 60)
    print("CONNECTION TEST PASSED!")
    print("=" * 60)
    return True


def show_token_instructions():
    """Show instructions to generate a new access token."""
    print("=" * 60)
    print("HOW TO GENERATE A NEW ACCESS TOKEN")
    print("=" * 60)
    print()
    print("Step 1: Open this URL in your browser:")
    print(f"  https://kite.zerodha.com/connect/login?v=3&api_key={API_KEY}")
    print()
    print("Step 2: Log in with your Zerodha credentials (Client ID: DS6203)")
    print()
    print("Step 3: After authorization, you'll be redirected to a URL like:")
    print("  http://127.0.0.1/?request_token=XXXXXXXX&action=login&status=success")
    print()
    print("Step 4: Copy the 'request_token' value from the URL")
    print()
    print("Step 5: Run this command to exchange it for an access token:")
    print("  python generate_access_token.py")
    print()
    print("Or manually exchange using:")
    print("  python exchange_token.py <request_token>")


if __name__ == "__main__":
    success = test_connection()
    sys.exit(0 if success else 1)
