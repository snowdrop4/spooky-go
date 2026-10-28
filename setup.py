from setuptools import setup
from setuptools_rust import Binding, RustExtension

setup(
    rust_extensions=[
        RustExtension(
            "spooky_go",
            binding=Binding.PyO3,
            debug=False,
            features=["python"],
            rustc_flags=["-Copt-level=3", "-Clto=fat"],
        )
    ],
    data_files=[("", ["spooky_go.pyi"])],
    zip_safe=False,
)
