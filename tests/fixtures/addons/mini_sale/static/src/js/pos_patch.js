import { patch } from "@web/core/utils/patch";
import { registry } from "@web/core/registry";
import { PaymentScreen } from "@point_of_sale/app/screens/payment_screen/payment_screen";

patch(PaymentScreen.prototype, {
    async validateOrder(isForceValidate) {
        return super.validateOrder(isForceValidate);
    },
});

class CustomPaymentScreen extends PaymentScreen {
    setup() {
        super.setup();
    }
}

registry.category("pos_screens").add("CustomPaymentScreen", CustomPaymentScreen);
