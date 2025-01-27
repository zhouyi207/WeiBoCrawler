from typing import Optional
from pydantic import BaseModel

class ListItem(BaseModel):
    mid : Optional[str]
    uid : Optional[str]
    personal_name : Optional[str]
    personal_href : Optional[str]
    weibo_href : Optional[str]
    publish_time : Optional[str]
    content_from : Optional[str]
    content_all : Optional[str]
    retweet_num : Optional[int]
    comment_num : Optional[int]
    star_num : Optional[int]


def process_list_json(list_json):
    return [ListItem.model_validate(item) for item in list_json]
    