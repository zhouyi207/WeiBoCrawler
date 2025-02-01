import streamlit as st
from util import get_body_data, db, BodyRecord, process_body_documents

cols = st.columns([7, 3, 2, 2, 2], vertical_alignment="bottom")
cols[0].text_input("搜索id列表(用空格分隔)", value="OEEV7wXHY Oj0PXme8I OiZre8dir Oj0zUmucE", key="ids")
cols[1].text_input("存储表名", value="test", key="table_name")

cols[-1].button("搜索", type="primary", key="body_button")

if st.session_state["body_button"]:
    ids = st.session_state["ids"].split()
    if st.session_state["table_name"] == "" or ids == []:
        st.warning("搜索id列表和存储表名不能为空")
    else:
        with st.spinner("搜索中(进展在控制台)..."):
            res_ids = get_body_data(id=ids, table_name=st.session_state["table_name"])
        with st.spinner("导入中(进展在控制台)..."):
            records = db.sync_get_records_by_ids(BodyRecord, res_ids)
            documents = [record.json_data for record in records]
            st.session_state["body"] = process_body_documents(documents)

if "body" in st.session_state:
    st.dataframe(st.session_state["body"])