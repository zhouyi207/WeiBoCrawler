import streamlit as st

# 在使用绝对路径的时候, 只有初次能够成功！如果修改页面就会报错. 
# 使用相对路径的话不是基于项目的, 而是基于运行 streamlit run main.py 的路径.


st.set_page_config(
    page_title="微博爬虫数据分析",
    page_icon="💻",
    layout="wide",
    initial_sidebar_state="expanded",
)

test_page = st.Page("./web_pages/database/search_database.py", title="数据展示", icon=":material/add_circle:")

demo_page = st.Page("./web_pages/test.py", title="测试", icon=":material/add_circle:")


pg = st.navigation({
    "数据库": [test_page],
    "测试": [demo_page],
})

pg.run()