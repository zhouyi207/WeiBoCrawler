import streamlit as st
import toml
from util import cookies_config, config_path, get_qr_Info, get_qr_status
from datetime import datetime
from threading import Thread
from streamlit.runtime.scriptrunner import add_script_run_ctx, get_script_run_ctx

if 'Thread' not in st.session_state:
    st.session_state["Thread"] = None

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


def get_cookies(client, login_signin_url, qrid):
    cookies = get_qr_status(client, login_signin_url, qrid)
    if cookies is None:
        st.error("获取 cookies 失败!!!!!!!")
    else:
        set_cookies(cookies)
    client.close()


@st.dialog("使用微博APP扫码登录")
def scan_code():
    if st.session_state["Thread"] is not None and st.session_state["Thread"].is_alive():
        st.image(image=st.session_state["image"])
    else:
        image, client, login_signin_url, qrid = get_qr_Info()
        st.session_state["image"] = image
        st.image(image=image)

        st.session_state["Thread"] = Thread(target=get_cookies, args=(client, login_signin_url, qrid))
        add_script_run_ctx(st.session_state["Thread"], get_script_run_ctx())
        st.session_state["Thread"].start()

cols = st.columns([1, 1, 15])
cols[0].button("更新", key="update", on_click=scan_code, type="secondary", use_container_width=True)
if cols[1].button("刷新", key="rerun", type="secondary", use_container_width=True):
    st.rerun()
st.write(cookies_config)