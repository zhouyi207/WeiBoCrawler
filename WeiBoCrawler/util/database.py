import toml
from .path import module_path, config_path, Path
from pydantic import BaseModel, field_validator


class DatabaseConfig(BaseModel):
    path: str

    @field_validator('path')
    def modify_module_path(cls, value):
        if Path(value).is_absolute():
            return str(value)
        else:
            return str(module_path / value)
        

database_config = DatabaseConfig.model_validate(toml.load(config_path)["database"])
