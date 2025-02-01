import toml
from pydantic import BaseModel
from .path import config_path

class CookiesConfig(BaseModel):
    """这个类主要用来保存 Cookies

    Attributes:
        cookies (dict): 微博的cookies
        cookies_info (datetime): 更新时间    
    """
    cookies: dict
    cookies_info: dict

cookies_config = CookiesConfig.model_validate(toml.load(config_path))
