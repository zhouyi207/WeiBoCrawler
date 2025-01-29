import streamlit as st
from tinydb import TinyDB
from src.WeiBoCrawler.util import database_config


db = TinyDB(database_config.list)

st.write(db.tables())

