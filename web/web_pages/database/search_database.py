import sys
sys.path.append(".")

import streamlit as st
from tinydb import TinyDB
from WeiBoCrawler.util import database_config
from WeiBoCrawler.parse import process_body_documents, process_list_documents, process_comment_documents


database_path = {
    "list": database_config.list,
    "body": database_config.body,
    "comment1": database_config.comment1,
    "comment2": database_config.comment2,
}

st.header("数据库查询🍎")

cols = st.columns([2, 5, 10])

with cols[0]:
    select_database = st.selectbox("选择数据库", ["list", "body", "comment1", "comment2"], index=0)
    db = TinyDB(database_path[select_database])

with cols[1]:
    select_table = st.selectbox("选择数据表", db.tables(), index=0)
    table = db.table(select_table)


if select_database in ["comment1", "comment2"]:
    pass
    # st.write(process_comment_table(table))

if select_database == "body":
    # st.write(process_body_table(table))
    pass

if select_database == "list":
    # st.write(process_list_table(table))
    pass