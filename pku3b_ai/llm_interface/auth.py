# llm_interface/auth.py
import os
import json
from pathlib import Path
from getpass import getpass
from pku3b_py import PyClient, PyBlackboard

CACHE_PATH = Path.home() / ".pku3b_ai_login.json"

def load_credentials():
    if CACHE_PATH.exists():
        with open(CACHE_PATH, "r", encoding="utf-8") as f:
            data = json.load(f)
            return data["user"], data["pwd"]
    return None, None

def save_credentials(user: str, pwd: str):
    with open(CACHE_PATH, "w", encoding="utf-8") as f:
        json.dump({"user": user, "pwd": pwd}, f)

def load_or_login() -> PyBlackboard:
    client = PyClient()
    user, pwd = load_credentials()

    if not user or not pwd:
        print("🔐 第一次登录，请输入账号密码（将保存在 ~/.pku3b_ai_login.json）")
        user = input("用户名: ")
        pwd = getpass("密码（输入时不可见）: ")
        save_credentials(user, pwd)

    try:
        bb = client.login_blackboard(user, pwd)
        print(f"✅ 登录成功：{user}")
        return bb
    except Exception as e:
        print(f"❌ 登录失败：{e}")
        CACHE_PATH.unlink(missing_ok=True)
        raise