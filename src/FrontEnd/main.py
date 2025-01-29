import sys
import os
import streamlit as st
from tinydb import TinyDB

from ..WeiBoCrawler.database import database_config
from..WeiBoCrawler.database import process_list_table

# 将父目录添加到 sys.path 中
parent_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.append(parent_dir)


db = TinyDB(database_config.list)

st.write(db.tables())

table_name = "#姜萍已达到数学系本科生的水平#"
table = db.table(table_name)

df = process_list_table(table)
st.dataframe(df)



db = TinyDB(database_config.body)
st.write(db.tables())

table_name = "Okh1b1YeY"
table = db.table(table_name)

st.write(table.all())