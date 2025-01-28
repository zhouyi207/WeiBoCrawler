from tinydb import TinyDB
from ..database.util import database_config


db_list = TinyDB(database_config.list)
db_body = TinyDB(database_config.body)
db_comment1 = TinyDB(database_config.comment1)
db_comment2 = TinyDB(database_config.comment2)