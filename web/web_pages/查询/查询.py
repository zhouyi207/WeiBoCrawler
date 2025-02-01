from util import db
import streamlit as st
import pandas as pd

cols = st.columns([10, 1], vertical_alignment="bottom")

cols[0].text_input(label="sql(切记这里要记得写limit，不然卡死你)", placeholder="写sql语句", value="select * from BodyRecord limit 100 offset 10;", key="sql")
cols[1].button("执行sql", key="sql_button")

if st.session_state.get("sql_button"):
    df = pd.DataFrame(db.sql(st.session_state.sql))
    st.session_state["sql_result"] = df
    

if "sql_result" in st.session_state:
    st.write(st.session_state["sql_result"])