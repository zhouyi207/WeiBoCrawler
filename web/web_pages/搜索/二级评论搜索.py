import streamlit as st
from util import get_comment2_data, db, Comment2Record, process_comment_documents

cols = st.columns([4, 4, 3, 1, 2, 2], vertical_alignment="bottom")
cols[0].text_input("uid 列表(用空格分隔)", value="1644114654 1644114654 1644114654", key="uid")
cols[1].text_input("mid 列表(用空格分隔)", value="5045280045531535 5045270515551948 5045277713760776", key="mid")
cols[2].text_input("存储表名", value="test", key="table_name")

cols[-1].button("搜索", type="primary", key="comment2_button")

if st.session_state["comment2_button"]:
    uids = st.session_state["uid"].split()
    mids = st.session_state["mid"].split()

    if st.session_state["table_name"] == "" or mids == [] or uids == []:
        st.warning("uid列表，mid列表存储表名不能为空")
    elif len(mids) != len(uids):
        st.warning("uid列表和mid列表长度必须一致")
    else:
        with st.spinner("搜索中(进展在控制台)..."):
            res_ids = get_comment2_data(uid=uids, mid=mids, table_name=st.session_state["table_name"])
        with st.spinner("导入中(进展在控制台)..."):
            records = db.sync_get_records_by_ids(Comment2Record, res_ids)
            documents = [record.json_data for record in records]
            st.session_state["comment2"] = process_comment_documents(documents)

if "comment2" in st.session_state:
    st.dataframe(st.session_state["comment2"])