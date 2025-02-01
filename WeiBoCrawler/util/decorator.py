from .log import logging
from typing import Callable
import httpx
from pydantic import validate_call

def custom_validate_call(func: Callable) -> Callable:
    return validate_call(func, config={"arbitrary_types_allowed": True}, validate_return=True)

def log_function_params(logger: logging.Logger=logging):
    """记录函数的参数和返回值

    Args:
        func (Callable): 需要装饰的函数
           
    Returns:
        Callable: 装饰后的函数
    """
    def log_function_params_(func:Callable) -> Callable:
        def wrapper(*args, **kwargs):
            # 记录函数名和参数
            args_repr = [repr(a) for a in args]
            kwargs_repr = [f"{k}={v!r}" for k, v in kwargs.items()]
            signature = ", ".join(args_repr + kwargs_repr)
            logger.info(f"Calling Function {func.__name__}({signature})")
            
            # 调用原函数
            result = func(*args, **kwargs)
            
            # 记录返回值
            logger.info(f"Function {func.__name__} returned {result!r}")
            return result
        return wrapper
    return log_function_params_


def retry_timeout_decorator(func: Callable) -> Callable:
    """超时重试装饰器

    Args:
        retry_times (int): 重试次数. Defaults to 3.

    Returns:
        Callable: 装饰后的函数
    """
    retry_times = 3
    def wrapper(*args, **kwargs):
        attempts = 0
        while attempts < retry_times:
            try:
                return func(*args, **kwargs)
            except httpx.TimeoutException as e:
                attempts += 1
                if attempts < retry_times:
                    logging.warning(f"请求超时，正在进行第 {attempts} 次重试...")
                else:
                    logging.error(f"请求超时，重试次数已达到最大值，请检查网络连接或重试次数！错误原因{e}")
    return wrapper


def retry_timeout_decorator_asyncio(func: Callable) -> Callable:
    """超时重试装饰器(异步)

    Args:
        retry_times (int): 重试次数. Defaults to 3.

    Returns:
        Callable: 装饰后的函数
    """
    retry_times = 3
    async def wrapper(*args, **kwargs):  # 将 wrapper 改为异步函数
        attempts = 0
        while attempts < retry_times:
            try:
                return await func(*args, **kwargs)  # 调用异步函数并使用 await
            except httpx.TimeoutException as e:
                attempts += 1
                if attempts < retry_times:
                    logging.warning(f"请求超时，正在进行第 {attempts} 次重试...")
                else:
                    logging.error(f"请求超时，重试次数已达到最大值，请检查网络连接或重试次数！错误原因{e}")
    return wrapper