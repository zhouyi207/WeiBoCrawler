from pathlib import Path


module_path = Path(__file__).parent.parent

database_config_path = module_path / "./config.toml"
request_params_path = module_path / "./request/request.toml"