import streamlit as st
from util import get_comment1_data, db, Comment1Record, process_comment_documents

cols = st.columns([4, 4, 3, 1, 2, 2], vertical_alignment="bottom")
cols[0].text_input("uid 列表(用空格分隔)", value="2035895904 1749277070", key="uid")
cols[1].text_input("mid 列表(用空格分隔)", value="5096904217856018 5045463240409185", key="mid")
cols[2].text_input("存储表名", value="test", key="table_name")

cols[-1].button("搜索", type="primary", key="comment1_button")

if st.session_state["comment1_button"]:
    uids = st.session_state["uid"].split()
    mids = st.session_state["mid"].split()

    if st.session_state["table_name"] == "" or mids == [] or uids == []:
        st.warning("uid列表，mid列表存储表名不能为空")
    elif len(mids) != len(uids):
        st.warning("uid列表和mid列表长度必须一致")
    else:
        with st.spinner("搜索中(进展在控制台)..."):
            res_ids = get_comment1_data(uid=uids, mid=mids, table_name=st.session_state["table_name"])
        with st.spinner("导入中(进展在控制台)..."):
            records = db.sync_get_records_by_ids(Comment1Record, res_ids)
            documents = [record.json_data for record in records]
            st.session_state["comment1"] = process_comment_documents(documents)

if "comment1" in st.session_state:
    st.dataframe(st.session_state["comment1"])