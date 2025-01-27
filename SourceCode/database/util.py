from pydantic import BaseModel
import toml


class Database_Config(BaseModel):
    list: str
    body: str
    comment1: str
    comment2: str


database_config = Database_Config.model_validate(toml.load("./SourceCode/config.toml")["database"])