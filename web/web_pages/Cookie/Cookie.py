import streamlit as st
import toml
from util import cookies_config, get_cookies, config_path
from datetime import datetime


def set_cookies():
    cookies = get_cookies()
    if cookies is not None:
        cookies_config.cookies.update(cookies)
        cookies_config.cookies_info["update_time"] = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
        config_data = toml.load(config_path)

        config_data["cookies"].update(cookies_config.cookies_info)
        config_data["cookies_info"].update(cookies_config.cookies_info)

        with open(config_path, "w", encoding="utf-8") as f:
            toml.dump(config_data, f)
    else:
        st.error("获取 cookies 失败!!!!!!!")


st.button("更新", key="update", on_click=set_cookies, type="primary")
st.write(cookies_config)