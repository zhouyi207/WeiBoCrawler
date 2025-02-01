import streamlit as st

# 在使用绝对路径的时候, 只有初次能够成功！如果修改页面就会报错. 
# 使用相对路径的话不是基于项目的, 而是基于运行 streamlit run main.py 的路径.


st.set_page_config(
    page_title="微博爬虫数据分析",
    page_icon="💻",
    layout="wide",
    initial_sidebar_state="expanded",
)


pg = st.navigation({
    "Cookie": [
        st.Page("./web_pages/Cookie/Cookie.py", title="Cookie", icon=":material/add_circle:")
    ],
    "下载": [
        st.Page("./web_pages/搜索/列表搜索.py", title="列表搜索", icon=":material/add_circle:"),
        st.Page("./web_pages/搜索/详细页搜索.py", title="详细页搜索", icon=":material/add_circle:"),
        st.Page("./web_pages/搜索/一级评论搜索.py", title="一级评论搜索", icon=":material/add_circle:"),
        st.Page("./web_pages/搜索/二级评论搜索.py", title="二级评论搜索", icon=":material/add_circle:"),
    ],
    "查询": [
        st.Page("./web_pages/查询/查询.py", title="SQL语句查询", icon=":material/add_circle:")
    ],
})

pg.run()