import os
import pandas as pd
from rich.progress import track
from utils.__init__ import *
import logging


logging.basicConfig(level=logging.INFO)


class WBParser:
    def __init__(self, cookie):
        self.cookie = cookie

        os.makedirs("./WBData", exist_ok=True)
        os.makedirs("./WBData/Comments_level_1", exist_ok=True)
        os.makedirs("./WBData/Comments_level_2", exist_ok=True)
        self.main_body_filepath = "./WBData/demo.csv"
        self.comments_level_1_filename = "./WBData/demo_comments_one.csv"
        self.comments_level_2_filename = "./WBData/demo_comments_two.csv"
        self.comments_level_1_dirpath = "./WBData/Comments_level_1/"
        self.comments_level_2_dirpath = "./WBData/Comments_level_2/"

    def get_main_body(self, q, kind):
        data = get_all_main_body(q, kind, self.cookie)
        data = data.reset_index(drop=True).astype(str).drop_duplicates()
        data.to_csv(self.main_body_filepath, encoding="utf_8_sig")

    def get_comments_level_one(self):
        data_list = []

        assert os.path.exists(self.main_body_filepath), "没有找到主题内容，请先获取主体内容"
        main_body = pd.read_csv(self.main_body_filepath, index_col=0)
        logging.info(f"主体内容一共有{main_body.shape[0]:5d}个，现在开始解析...")

        try:
            for ix in track(range(main_body.shape[0]), description=f"解析中..."):
                uid = int(float(main_body.iloc[ix]["uid"]))
                mid = int(float(main_body.iloc[ix]["mid"]))
                final_file_path = f"{self.comments_level_1_dirpath}{uid}_{mid}.csv"

                if os.path.exists(final_file_path):
                    length = pd.read_csv(final_file_path).shape[0]
                    if length > 0:
                        continue

                data = get_all_level_one(uid=uid, mid=mid, cookie=self.cookie)
                data.drop_duplicates(inplace=True)
                data.to_csv(final_file_path, encoding="utf_8_sig")
                data_list.append(data)
            logging.info(f"主体内容一共有{main_body.shape[0]:5d}个，已经解析完毕！")
        except:
            logging.error(f"由于cookie失效等原因，一共解析主体内容{len(data_list):5d}个！")

        data = pd.concat(data_list).reset_index(drop=True).astype(str).drop_duplicates()
        data.to_csv(self.comments_level_1_filename)

    def get_comments_level_two(self):
        data_list = []

        # 获取一级评论
        if os.path.exists(self.comments_level_1_filename):
            comments_level_1_data = pd.read_csv(self.comments_level_1_filename, index_col=0)
        else:
            file_list = [self.comments_level_1_dirpath + item for item in os.listdir(self.comments_level_1_dirpath) if item.endswith('.csv')]
            assert len(file_list) > 0, "没有找到一级评论文件，请先获取一级评论"
            comments_level_1_data = pd.concat([pd.read_csv(file) for file in file_list]).reset_index(drop=True).astype(str).drop_duplicates()

        logging.info(
            f"一级评论一共有{comments_level_1_data.shape[0]:5d}个，现在开始解析..."
        )
        try:
            for ix in track(
                range(comments_level_1_data.shape[0]), description=f"解析中..."
            ):
                main_body_uid = int(float(comments_level_1_data.iloc[ix]["main_body_uid"]))
                mid = int(float(comments_level_1_data.iloc[ix]["mid"]))
                final_file_path = (
                    f"{self.comments_level_2_dirpath}{main_body_uid}_{mid}.csv"
                )

                if os.path.exists(final_file_path):
                    length = pd.read_csv(final_file_path).shape[0]
                    if length > 0:
                        continue

                data = get_all_level_two(uid=main_body_uid, mid=mid, cookie=self.cookie)
                data.drop_duplicates(inplace=True)
                data.to_csv(final_file_path, encoding="utf_8_sig")
                data_list.append(data)
            logging.info(f"一级评论一共有{comments_level_1_data.shape[0]:5d}个，已经解析完毕！")
        except:
            logging.error(f"由于cookie失效等原因，一共解析一级评论{len(data_list):5d}个！")

        data = pd.concat(data_list).reset_index(drop=True).astype(str).drop_duplicates()
        data.to_csv(self.comments_level_2_filename)


if __name__ == "__main__":
    q = "#姜萍中考621分却上中专的原因#"  # 话题
    kind = "综合"  # 综合，实时，热门，高级
    cookie = ""
    wbparser = WBParser(cookie)

    # 获取主题内容
    # wbparser.get_main_body(q, kind)
    # 获取一级评论
    # wbparser.get_comments_level_one()
    # 获取二级评论
    # wbparser.get_comments_level_two()