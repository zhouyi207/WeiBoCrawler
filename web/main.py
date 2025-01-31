import streamlit as st

# 主页设置
st.set_page_config(
    page_title="微博爬虫数据分析",
    page_icon="💻",
    layout="wide",
    initial_sidebar_state="expanded",
)

# siderbar
pg = st.navigation({
    "搜索": [
        st.Page("./web_pages/search/search.py", title="搜索", icon=":material/add_circle:"),
    ],
    "数据库": [
        st.Page("./web_pages/database/search_database.py", title="数据展示", icon=":material/add_circle:"),
    ],
    "测试": [
        st.Page("./web_pages/test.py", title="测试", icon=":material/add_circle:")
    ],
})

pg.run() 