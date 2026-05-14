from odoo import fields, models


class SaleOrder(models.Model):
    _inherit = "sale.order"

    x_reference = fields.Char(compute="_compute_x_reference")

    def action_confirm(self):
        result = super().action_confirm()
        self._after_confirm_hook()
        return result

    def _after_confirm_hook(self):
        return True

