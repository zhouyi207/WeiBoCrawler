from ..parse.parse_list_html import parse_list_html
from ..request.get_list_request import get_list_response
from tinydb import TinyDB


def get_list_data(search_for, page_index):
    db = TinyDB("./list.json")
    table = db.table(search_for)
    resp = get_list_response(search_for, page_index)
    table.insert_multiple(parse_list_html(resp.text))
    db.close()

