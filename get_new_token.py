#!/usr/bin/env python3
"""
Simple script to exchange request_token for access_token.

Usage:
  python get_new_token.py <request_token>

Or run without arguments for instructions.
"""

import sys
import hashlib
import requests

API_KEY = "7zks6tfai2vje1at"
API_SECRET = "gt8hvnuiwtz7w8a3dwd8tcqnvguk31el"


def generate_checksum(api_key, request_token, api_secret):
    """Generate SHA-256 checksum."""
    data = api_key + request_token + api_secret
    return hashlib.sha256(data.encode()).hexdigest()


def exchange_token(request_token):
    """Exchange request_token for access_token."""
    print(f"Exchanging request_token: {request_token[:8]}***")

    checksum = generate_checksum(API_KEY, request_token, API_SECRET)

    response = requests.post(
        "https://api.kite.trade/session/token",
        data={
            "api_key": API_KEY,
            "request_token": request_token,
            "checksum": checksum
        },
        headers={"X-Kite-Version": "3"},
        timeout=30
    )

    if response.status_code == 200:
        data = response.json()
        if data.get("status") == "success":
            access_token = data["data"]["access_token"]
            user = data["data"]

            print()
            print("=" * 60)
            print("SUCCESS!")
            print("=" * 60)
            print(f"User: {user.get('user_name')} ({user.get('user_id')})")
            print(f"Email: {user.get('email')}")
            print()
            print("Your new ACCESS TOKEN:")
            print("-" * 60)
            print(access_token)
            print("-" * 60)
            print()
            print("Update config/zerodha_credentials.toml with:")
            print(f'access_token = "{access_token}"')
            print()

            # Auto-update the config file
            try:
                config_path = "config/zerodha_credentials.toml"
                with open(config_path, 'r') as f:
                    content = f.read()

                # Replace the access_token line
                lines = content.split('\n')
                for i, line in enumerate(lines):
                    if line.strip().startswith('access_token = '):
                        lines[i] = f'access_token = "{access_token}"'
                        break

                with open(config_path, 'w') as f:
                    f.write('\n'.join(lines))

                print(f"Config file updated automatically!")
            except Exception as e:
                print(f"Note: Could not auto-update config: {e}")

            return access_token
        else:
            print(f"API Error: {data}")
    else:
        print(f"HTTP {response.status_code}: {response.text}")

    return None


def show_instructions():
    """Show how to get request_token."""
    print("=" * 60)
    print("ZERODHA ACCESS TOKEN GENERATOR")
    print("=" * 60)
    print()
    print("Step 1: Open this URL in your browser:")
    print()
    print(f"  https://kite.zerodha.com/connect/login?v=3&api_key={API_KEY}")
    print()
    print("Step 2: Log in with your Zerodha credentials")
    print("        Client ID: DS6203")
    print()
    print("Step 3: After authorization, you'll be redirected to a URL like:")
    print("  http://127.0.0.1/?request_token=XXXXXXXX&action=login&status=success")
    print()
    print("Step 4: Copy the request_token value and run:")
    print("  python get_new_token.py YOUR_REQUEST_TOKEN")
    print()


if __name__ == "__main__":
    if len(sys.argv) < 2:
        show_instructions()
        sys.exit(0)

    request_token = sys.argv[1].strip()

    if not request_token:
        print("Error: request_token cannot be empty")
        sys.exit(1)

    result = exchange_token(request_token)
    sys.exit(0 if result else 1)
