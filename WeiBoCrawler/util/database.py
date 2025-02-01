import toml
from .path import module_path, database_config_path, Path
from pydantic import BaseModel, field_validator


class Database_Config(BaseModel):
    path: str

    @field_validator('path')
    def modify_module_path(cls, value):
        if Path(value).is_absolute():
            return str(value)
        else:
            return str(module_path / value)
        

database_config = Database_Config.model_validate(toml.load(database_config_path)["database"])
