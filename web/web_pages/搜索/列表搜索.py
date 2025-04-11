import streamlit as st
from util import get_list_data, db, BodyRecord, process_list_documents
from datetime import date

cols = st.columns([3, 3, 1, 1, 2, 2, 2, 2], vertical_alignment="bottom")
cols[0].text_input("搜索内容(话题需要在前后加上#)", value="姜平", key="search_for")
cols[1].text_input("存储表名", value="test", key="table_name")
cols[2].selectbox("搜索类型", options=["综合", "实时", "高级"], key="kind")
cols[3].selectbox("筛选条件", options=["综合", "热度", "原创"], key="advanced_kind", disabled=st.session_state["kind"]!= "高级")
cols[4].date_input("起始时间", value="today", min_value=date(year=2000, month=1, day=1), key="start", disabled=st.session_state["kind"]!= "高级")
cols[5].date_input("结束时间", value="today", key="end", min_value=date(year=2000, month=1, day=1), disabled=st.session_state["kind"]!= "高级")

cols[-1].button("搜索", type="primary", key="list_button")

if st.session_state["list_button"]:
    if st.session_state["search_for"] == "" or st.session_state["table_name"] == "":
        st.warning("搜索内容和存储表名不能为空")
    else:
        with st.spinner("搜索中(进展在控制台)..."):
            res_ids = get_list_data(search_for=st.session_state["search_for"], table_name=st.session_state["table_name"], 
                      kind=st.session_state["kind"], advanced_kind=st.session_state["advanced_kind"], time_start=st.session_state["start"], time_end=st.session_state["end"])
        with st.spinner("导入中(进展在控制台)..."):
            records = db.sync_get_records_by_ids(BodyRecord, res_ids)
            documents = [record.json_data for record in records]
            st.session_state["list"] = process_list_documents(documents)

if "list" in st.session_state:
    st.dataframe(st.session_state["list"])