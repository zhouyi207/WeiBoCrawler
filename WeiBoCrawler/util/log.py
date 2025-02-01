import logging
from .path import module_path


# 配置日志
logging.basicConfig(
    filename=module_path / "./app.log",
    level=logging.INFO, 
    format='%(asctime)s - %(levelname)s - %(name)s - %(message)s',
    encoding="utf-8",
)