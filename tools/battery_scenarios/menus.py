"""Lane: menu screens, the Quick Mission creator, loadout and GUI captures."""
from battery import Scenario


def scenarios() -> list[Scenario]:
    return [
        Scenario(name="menus-validate-creator", lane="menus", args=["--validate-creator", "--no-audio"], timeout=600),
    ]
