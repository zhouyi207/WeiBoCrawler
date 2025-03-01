import streamlit as st
import toml
from util import cookies_config, config_path, get_qr_Info, get_qr_status
from datetime import datetime


def set_cookies(cookies):
    if cookies is not None:
        cookies_config.cookies.update(cookies)
        cookies_config.cookies_info["update_time"] = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
        config_data = toml.load(config_path)

        config_data["cookies"].update(cookies_config.cookies)
        config_data["cookies_info"].update(cookies_config.cookies_info)

        with open(config_path, "w", encoding="utf-8") as f:
            toml.dump(config_data, f)
    else:
        st.error("获取 cookies 失败!!!!!!!")


@st.fragment
def get_cookies(client, login_signin_url, qrid):
    cookies = get_qr_status(client, login_signin_url, qrid)
    if cookies is None:
        st.error("获取 cookies 失败!!!!!!!")
    else:
        set_cookies(cookies)
    client.close()


@st.dialog("使用微博APP扫码登录")
def scan_code():
    image, client, login_signin_url, qrid = get_qr_Info()
    st.image(image=image)
    get_cookies(client, login_signin_url, qrid)


st.button("更新", key="update", on_click=scan_code, type="primary")
st.write(cookies_config)

st.write(st.session_state)