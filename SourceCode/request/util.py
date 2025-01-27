import toml
from datetime import datetime
from pydantic import BaseModel
from typing import Optional

class RequestParams(BaseModel):
    body_headers: dict
    comment1_buildComments_headers: dict
    comment1_rum_headers: dict
    cookies: dict
    update_time: datetime = Optional[datetime]

requestparams = RequestParams.model_validate(toml.load("./SourceCode/request/request.toml"))